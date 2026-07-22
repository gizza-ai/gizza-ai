//! fuzzy-dedupe core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps.
//!
//! Finds and removes **near-duplicate** rows/strings via fuzzy similarity — typos,
//! casing, and spacing differences that exact de-duplication misses — and returns
//! the **cleaned dataset** with one representative kept per near-duplicate group.
//!
//! Grouping is a single greedy pass: each row's key is compared against existing
//! group seeds with a normalized (bounded) Levenshtein edit-distance ratio in
//! 0–100; a row joins a group when its similarity is at least `threshold`. The
//! survivor kept per group is chosen by `keep` (first / longest / most_frequent).
//!
//! This is the de-duplication counterpart to `cluster-similar-values` (which emits
//! a canonicalization *mapping*); here the primary output is the de-duplicated data.

use std::collections::HashMap;

use serde::Serialize;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve a single `columns` token (a 1-based index or, when there's a header, a
/// name) to a 0-based column index.
fn resolve_column(
    token: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<usize, String> {
    let t = token.trim();
    if let Ok(n) = t.parse::<usize>() {
        if n == 0 {
            return Err("column indices are 1-based (>= 1)".into());
        }
        if n > width {
            return Err(format!("column index {n} is out of range (data has {width} columns)"));
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => match h.iter().position(|c| c == t) {
            Some(p) => Ok(p),
            None => Err(format!("column '{t}' not found in the header")),
        },
        None => Err(format!(
            "column '{t}' is not a number and there is no header to match names (enable `header` or use an index)"
        )),
    }
}

/// Parse the `columns` spec (comma-separated names/indices) into 0-based indices.
/// Empty → `None`, meaning "match the whole row".
fn resolve_columns(
    columns: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<Option<Vec<usize>>, String> {
    if columns.trim().is_empty() {
        return Ok(None);
    }
    let mut out = Vec::new();
    for tok in columns.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        out.push(resolve_column(tok, header, width)?);
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

/// Normalize a value for *comparison only* (never for display): collapse runs of
/// whitespace and trim when `spacing`, lowercase when `case`.
fn normalize(s: &str, case: bool, spacing: bool) -> String {
    let mut r = if spacing {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        s.to_string()
    };
    if case {
        r = r.to_lowercase();
    }
    r
}

/// Bounded Levenshtein edit distance. Returns `max + 1` once every cell in a row
/// exceeds `max` (the caller only cares whether the distance clears a threshold).
fn levenshtein_bounded(a: &[char], b: &[char], max: usize) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let gap = a.len().abs_diff(b.len());
    if gap > max {
        return max + 1;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        let mut row_min = curr[0];
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            let v = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
            curr[j + 1] = v;
            if v < row_min {
                row_min = v;
            }
        }
        if row_min > max {
            return max + 1;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Similarity of two already-normalized strings in 0..=100 (100 = identical).
fn similarity(a: &str, b: &str, threshold: f64) -> f64 {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let m = ca.len().max(cb.len());
    if m == 0 {
        return 100.0;
    }
    let max_dist = (m as f64 * (1.0 - threshold / 100.0)).floor() as usize;
    let d = levenshtein_bounded(&ca, &cb, max_dist);
    (1.0 - d as f64 / m as f64) * 100.0
}

/// One input row that made it past the blank-row filter.
struct Member {
    fields: Vec<String>,
    key_raw: String,
    order: usize,
}

struct Group {
    seed_norm: String,
    members: Vec<Member>,
}

#[derive(Serialize)]
struct OutGroup {
    kept: String,
    removed: Vec<String>,
    size: usize,
}

#[derive(Serialize)]
struct Out {
    threshold: f64,
    keep: String,
    total_rows: usize,
    kept_rows: usize,
    removed_rows: usize,
    near_duplicate_groups: usize,
    groups: Vec<OutGroup>,
}

/// Render one record back to a delimited single line (no trailing newline),
/// quoting fields as CSV rules require.
fn render_record(fields: &[String], delim: u8) -> String {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    wtr.write_record(fields).ok();
    wtr.flush().ok();
    let bytes = wtr.into_inner().unwrap_or_default();
    let s = String::from_utf8_lossy(&bytes);
    s.trim_end_matches(['\n', '\r']).to_string()
}

/// The compound key for a member: the selected columns joined, or the whole row.
fn member_key(fields: &[String], cols: &Option<Vec<usize>>) -> String {
    match cols {
        Some(idxs) => idxs
            .iter()
            .map(|&i| fields.get(i).map(String::as_str).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\u{1f}"),
        None => fields.join("\u{1f}"),
    }
}

/// Pick the surviving member index within a group per the `keep` strategy.
/// * `first` — earliest in input order.
/// * `longest` — longest full row (most information), ties → earliest.
/// * `most_frequent` — the exact key seen most often in the group, ties → earliest.
fn pick_survivor(members: &[Member], keep: &str) -> usize {
    match keep {
        "longest" => {
            let mut best = 0usize;
            let mut best_len = members[0].fields.join("").chars().count();
            for (i, m) in members.iter().enumerate().skip(1) {
                let len = m.fields.join("").chars().count();
                if len > best_len {
                    best_len = len;
                    best = i;
                }
            }
            best
        }
        "most_frequent" => {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for m in members {
                *counts.entry(m.key_raw.as_str()).or_insert(0) += 1;
            }
            let mut best = 0usize;
            let mut best_count = counts[members[0].key_raw.as_str()];
            for (i, m) in members.iter().enumerate().skip(1) {
                let c = counts[m.key_raw.as_str()];
                if c > best_count {
                    best_count = c;
                    best = i;
                }
            }
            best
        }
        // "first" and any unreachable value (validated by caller).
        _ => 0,
    }
}

/// Find and remove near-duplicate rows/strings, returning the de-duplicated data
/// (or the removed rows / a JSON report).
///
/// * `columns` — comma-separated header names or 1-based indices to match on;
///   blank matches the whole row/line.
/// * `threshold` — 0..=100; rows at or above this similarity are the "same".
/// * `keep` — which member of each group survives: `first`, `longest`,
///   `most_frequent`.
/// * `output` — `deduped` (the cleaned data), `removed` (only the dropped rows),
///   or `json` (structured groups + stats).
#[allow(clippy::too_many_arguments)]
pub fn dedupe(
    data: &str,
    columns: &str,
    delimiter: &str,
    header: bool,
    threshold: f64,
    keep: &str,
    normalize_case: bool,
    normalize_spacing: bool,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !(0.0..=100.0).contains(&threshold) {
        return Err(format!("threshold must be between 0 and 100, got {threshold}"));
    }
    if !matches!(keep, "first" | "longest" | "most_frequent") {
        return Err(format!(
            "keep must be one of first, longest, most_frequent — got '{keep}'"
        ));
    }
    if !matches!(output, "deduped" | "removed" | "json") {
        return Err(format!(
            "output must be one of deduped, removed, json — got '{output}'"
        ));
    }
    let delim = delim_byte(delimiter)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());

    let mut records: Vec<csv::StringRecord> = Vec::new();
    for rec in rdr.records() {
        records.push(rec.map_err(|e| format!("could not parse the data as delimited rows: {e}"))?);
    }
    if records.is_empty() {
        return Err("input is empty".into());
    }

    // Optional header row, kept aside and re-emitted verbatim.
    let header_rec: Option<csv::StringRecord> = if header { Some(records.remove(0)) } else { None };
    let width = records
        .iter()
        .map(|r| r.len())
        .chain(header_rec.iter().map(|h| h.len()))
        .max()
        .unwrap_or(0);
    let cols = resolve_columns(columns, header_rec.as_ref(), width)?;

    // Materialize members, skipping rows whose every field is blank.
    let mut members: Vec<Member> = Vec::new();
    for rec in &records {
        let fields: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        if fields.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let key_raw = member_key(&fields, &cols);
        let order = members.len();
        members.push(Member {
            fields,
            key_raw,
            order,
        });
    }
    if members.is_empty() {
        return Err("no non-empty rows to de-duplicate".into());
    }
    let total_rows = members.len();

    // Single greedy pass: compare each member's normalized key to existing seeds.
    let mut groups: Vec<Group> = Vec::new();
    for m in members {
        let norm = normalize(&m.key_raw, normalize_case, normalize_spacing);
        let hit = groups
            .iter()
            .position(|g| similarity(&norm, &g.seed_norm, threshold) >= threshold);
        match hit {
            Some(i) => groups[i].members.push(m),
            None => groups.push(Group {
                seed_norm: norm,
                members: vec![m],
            }),
        }
    }

    // Resolve the survivor of each group.
    struct Resolved {
        kept: Vec<String>,
        removed: Vec<(usize, Vec<String>)>, // (input order, fields)
        size: usize,
    }
    let resolved: Vec<Resolved> = groups
        .into_iter()
        .map(|g| {
            let survivor = pick_survivor(&g.members, keep);
            let size = g.members.len();
            let mut kept = Vec::new();
            let mut removed = Vec::new();
            for (i, m) in g.members.into_iter().enumerate() {
                if i == survivor {
                    kept = m.fields;
                } else {
                    removed.push((m.order, m.fields));
                }
            }
            Resolved { kept, removed, size }
        })
        .collect();

    let kept_rows = resolved.len();
    let removed_rows = total_rows - kept_rows;
    let near_duplicate_groups = resolved.iter().filter(|r| r.size > 1).count();

    match output {
        "deduped" => {
            let mut lines: Vec<String> = Vec::new();
            if let Some(h) = &header_rec {
                lines.push(render_record(
                    &h.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    delim,
                ));
            }
            for r in &resolved {
                lines.push(render_record(&r.kept, delim));
            }
            Ok(lines.join("\n"))
        }
        "removed" => {
            let mut dropped: Vec<(usize, Vec<String>)> =
                resolved.into_iter().flat_map(|r| r.removed).collect();
            dropped.sort_by_key(|(order, _)| *order);
            if dropped.is_empty() {
                return Ok("(no near-duplicate rows removed)".to_string());
            }
            let mut lines: Vec<String> = Vec::new();
            if let Some(h) = &header_rec {
                lines.push(render_record(
                    &h.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    delim,
                ));
            }
            for (_, fields) in &dropped {
                lines.push(render_record(fields, delim));
            }
            Ok(lines.join("\n"))
        }
        _ => {
            let groups_out: Vec<OutGroup> = resolved
                .into_iter()
                .map(|r| OutGroup {
                    kept: render_record(&r.kept, delim),
                    removed: r
                        .removed
                        .into_iter()
                        .map(|(_, f)| render_record(&f, delim))
                        .collect(),
                    size: r.size,
                })
                .collect();
            let out = Out {
                threshold,
                keep: keep.to_string(),
                total_rows,
                kept_rows,
                removed_rows,
                near_duplicate_groups,
                groups: groups_out,
            };
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_fuzzy_near_duplicates_keeping_first() {
        // Four New-York variants (typo, case, double space) + Boston twice. The
        // transposed "Nwe York" is 75% similar to "New York", so 70 merges it.
        let data = "New York\nnew york\nNew  York\nNwe York\nBoston\nBoston";
        let out = dedupe(data, "", ",", false, 70.0, "first", true, true, "deduped").unwrap();
        assert_eq!(out, "New York\nBoston");
    }

    #[test]
    fn keep_longest_picks_most_information() {
        let data = "Bob\nBobby\nBoby";
        let out = dedupe(data, "", ",", false, 60.0, "longest", true, true, "deduped").unwrap();
        assert_eq!(out, "Bobby");
    }

    #[test]
    fn keep_most_frequent_picks_dominant_spelling() {
        let data = "color\ncolour\ncolor\ncolor";
        let out = dedupe(data, "", ",", false, 80.0, "most_frequent", true, true, "deduped").unwrap();
        assert_eq!(out, "color");
    }

    #[test]
    fn csv_matches_on_selected_column_and_keeps_full_row() {
        let data = "company,amount\nAcme Inc,100\nacme inc,90\nAcme  Inc,80\nGlobex,50";
        let out = dedupe(data, "company", ",", true, 80.0, "first", true, true, "deduped").unwrap();
        assert_eq!(out, "company,amount\nAcme Inc,100\nGlobex,50");
    }

    #[test]
    fn removed_output_lists_dropped_rows_in_input_order() {
        let data = "New York\nnew york\nNwe York\nBoston";
        let out = dedupe(data, "", ",", false, 70.0, "first", true, true, "removed").unwrap();
        assert_eq!(out, "new york\nNwe York");
    }

    #[test]
    fn json_output_reports_stats_and_groups() {
        let data = "New York\nnew york\nBoston";
        let out = dedupe(data, "", ",", false, 80.0, "first", true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_rows"], 3);
        assert_eq!(v["kept_rows"], 2);
        assert_eq!(v["removed_rows"], 1);
        assert_eq!(v["near_duplicate_groups"], 1);
        assert_eq!(v["groups"][0]["kept"], "New York");
    }

    #[test]
    fn high_threshold_keeps_variants_separate() {
        // At 100 only identical-after-normalization rows merge.
        let data = "New York\nNwe York";
        let out = dedupe(data, "", ",", false, 100.0, "first", true, true, "deduped").unwrap();
        assert_eq!(out, "New York\nNwe York");
    }

    #[test]
    fn case_sensitivity_toggle_respected() {
        let data = "USA\nusa";
        let strict = dedupe(data, "", ",", false, 100.0, "first", false, true, "deduped").unwrap();
        assert_eq!(strict, "USA\nusa");
        let folded = dedupe(data, "", ",", false, 100.0, "first", true, true, "deduped").unwrap();
        assert_eq!(folded, "USA");
    }

    #[test]
    fn err_on_empty_input() {
        assert!(dedupe("   ", "", ",", false, 90.0, "first", true, true, "deduped").is_err());
    }

    #[test]
    fn err_on_out_of_range_threshold() {
        assert!(dedupe("a\nb", "", ",", false, 150.0, "first", true, true, "deduped").is_err());
    }

    #[test]
    fn err_on_bad_keep() {
        assert!(dedupe("a\nb", "", ",", false, 90.0, "newest", true, true, "deduped").is_err());
    }

    #[test]
    fn err_on_unknown_column() {
        let e = dedupe("a,b\n1,2", "nope", ",", true, 90.0, "first", true, true, "deduped")
            .unwrap_err();
        assert!(e.contains("not found in the header"), "got: {e}");
    }

    #[test]
    fn err_on_bad_output() {
        assert!(dedupe("a\nb", "", ",", false, 90.0, "first", true, true, "table").is_err());
    }
}
