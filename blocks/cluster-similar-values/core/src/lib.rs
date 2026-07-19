//! cluster-similar-values core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Fuzzy-clusters near-duplicate text values in one column of a CSV (or a plain
//! newline list). Values that differ only by typos, casing, or spacing are
//! grouped, and the most frequent original in each cluster is proposed as the
//! canonical form. Similarity is a normalized (bounded) Levenshtein edit-distance
//! ratio in 0–100; two values join a cluster when their similarity is at least
//! the `threshold`.

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

/// Resolve a single `column` token (a 1-based index or, when there's a header, a
/// name) to a 0-based column index. Empty → column 0 for single-column data, else
/// an error listing the header (ambiguous which column to cluster).
fn resolve_column(
    column: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<usize, String> {
    let t = column.trim();
    if t.is_empty() {
        if width <= 1 {
            return Ok(0);
        }
        return match header {
            Some(h) => Err(format!(
                "data has {width} columns — set `column` to one of: {}",
                h.iter().collect::<Vec<_>>().join(", ")
            )),
            None => Err(format!(
                "data has {width} columns — set `column` to a 1-based index (1..{width})"
            )),
        };
    }
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

/// Bounded Levenshtein edit distance. Stops early and returns `max + 1` once every
/// cell in a row exceeds `max` (the caller only cares whether the distance clears a
/// similarity threshold, so anything past the bound is equivalent).
fn levenshtein_bounded(a: &[char], b: &[char], max: usize) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    // Length gap alone already exceeds the budget.
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
/// Only computes precisely enough to decide against `threshold`: the max tolerated
/// edit distance is derived from the threshold, so the bounded distance suffices.
fn similarity(a: &str, b: &str, threshold: f64) -> f64 {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let m = ca.len().max(cb.len());
    if m == 0 {
        return 100.0;
    }
    // distance d gives sim = (1 - d/m)*100; sim >= threshold  <=>  d <= m*(1-threshold/100).
    let max_dist = (m as f64 * (1.0 - threshold / 100.0)).floor() as usize;
    let d = levenshtein_bounded(&ca, &cb, max_dist);
    (1.0 - d as f64 / m as f64) * 100.0
}

struct Member {
    value: String,
    count: usize,
}

struct Cluster {
    seed_norm: String,
    members: Vec<Member>,
    total: usize,
}

#[derive(Serialize)]
struct OutMember {
    value: String,
    count: usize,
}

#[derive(Serialize)]
struct OutCluster {
    canonical: String,
    size: usize,
    count: usize,
    members: Vec<OutMember>,
}

#[derive(Serialize)]
struct Out {
    threshold: f64,
    total_values: usize,
    unique_values: usize,
    cluster_count: usize,
    near_duplicate_clusters: usize,
    clusters: Vec<OutCluster>,
}

/// Cluster near-duplicate values in one column of `data` and render the result.
///
/// * `column` — a header name or 1-based index; blank uses the only column.
/// * `threshold` — 0..=100; values at or above this similarity join a cluster.
/// * `output` — "markdown" (default), "csv" (a flat original→canonical mapping),
///   or "json" (structured clusters).
#[allow(clippy::too_many_arguments)]
pub fn cluster(
    data: &str,
    column: &str,
    delimiter: &str,
    header: bool,
    threshold: f64,
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
    let fmt = match output.trim().to_ascii_lowercase().as_str() {
        "" | "markdown" | "md" => "markdown",
        "csv" => "csv",
        "json" => "json",
        other => return Err(format!("output must be markdown, csv, or json, got '{other}'")),
    };
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("input is empty".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(1);
    let hdr = if header { records.first() } else { None };
    let col = resolve_column(column, hdr, width)?;

    // Count distinct original values (first-seen order preserved), skipping the
    // header row and blank cells.
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut total_values = 0usize;
    for (i, rec) in records.iter().enumerate() {
        if header && i == 0 {
            continue;
        }
        let raw = rec.get(col).unwrap_or("");
        if raw.trim().is_empty() {
            continue;
        }
        total_values += 1;
        if !counts.contains_key(raw) {
            order.push(raw.to_string());
        }
        *counts.entry(raw.to_string()).or_insert(0) += 1;
    }
    if order.is_empty() {
        return Err("no non-empty values found in that column".into());
    }

    // Process the most frequent values first so a cluster's seed (and canonical) is
    // its most common member; ties break by first-seen order.
    let first_idx: HashMap<&str, usize> =
        order.iter().enumerate().map(|(i, v)| (v.as_str(), i)).collect();
    let mut uniques: Vec<(&str, usize)> =
        order.iter().map(|v| (v.as_str(), counts[v])).collect();
    uniques.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| first_idx[a.0].cmp(&first_idx[b.0]))
    });

    // Greedy clustering: each value joins the existing cluster whose seed is most
    // similar (>= threshold), else starts a new cluster.
    let mut clusters: Vec<Cluster> = Vec::new();
    for (value, count) in uniques {
        let norm = normalize(value, normalize_case, normalize_spacing);
        let mut best: Option<(usize, f64)> = None;
        for (ci, c) in clusters.iter().enumerate() {
            let sim = similarity(&norm, &c.seed_norm, threshold);
            if sim >= threshold && best.map(|(_, s)| sim > s).unwrap_or(true) {
                best = Some((ci, sim));
            }
        }
        match best {
            Some((ci, _)) => {
                clusters[ci].members.push(Member { value: value.to_string(), count });
                clusters[ci].total += count;
            }
            None => clusters.push(Cluster {
                seed_norm: norm,
                members: vec![Member { value: value.to_string(), count }],
                total: count,
            }),
        }
    }

    // Surface merged (near-duplicate) clusters first, then by frequency.
    clusters.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then_with(|| b.total.cmp(&a.total))
            .then_with(|| a.members[0].value.cmp(&b.members[0].value))
    });

    let unique_values = order.len();
    let cluster_count = clusters.len();
    let near_dupes = clusters.iter().filter(|c| c.members.len() > 1).count();

    match fmt {
        "json" => render_json(threshold, total_values, unique_values, near_dupes, &clusters),
        "csv" => render_csv(&clusters),
        _ => Ok(render_markdown(
            threshold,
            total_values,
            unique_values,
            cluster_count,
            near_dupes,
            &clusters,
        )),
    }
}

fn render_json(
    threshold: f64,
    total_values: usize,
    unique_values: usize,
    near_dupes: usize,
    clusters: &[Cluster],
) -> Result<String, String> {
    let out = Out {
        threshold,
        total_values,
        unique_values,
        cluster_count: clusters.len(),
        near_duplicate_clusters: near_dupes,
        clusters: clusters
            .iter()
            .map(|c| OutCluster {
                canonical: c.members[0].value.clone(),
                size: c.members.len(),
                count: c.total,
                members: c
                    .members
                    .iter()
                    .map(|m| OutMember { value: m.value.clone(), count: m.count })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&out).map_err(|e| format!("json error: {e}"))
}

fn render_csv(clusters: &[Cluster]) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
    wtr.write_record(["cluster", "original", "canonical", "count"])
        .map_err(|e| format!("CSV write error: {e}"))?;
    for (ci, c) in clusters.iter().enumerate() {
        let canonical = &c.members[0].value;
        for m in &c.members {
            wtr.write_record([
                &(ci + 1).to_string(),
                &m.value,
                canonical,
                &m.count.to_string(),
            ])
            .map_err(|e| format!("CSV write error: {e}"))?;
        }
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
}

fn render_markdown(
    threshold: f64,
    total_values: usize,
    unique_values: usize,
    cluster_count: usize,
    near_dupes: usize,
    clusters: &[Cluster],
) -> String {
    let thr = if threshold.fract() == 0.0 {
        format!("{}", threshold as i64)
    } else {
        format!("{threshold}")
    };
    let mut s = String::new();
    s.push_str("# Value clusters\n\n");
    s.push_str(&format!(
        "{total_values} values, {unique_values} unique → {cluster_count} clusters ({near_dupes} with near-duplicates) at threshold {thr}%.\n"
    ));

    s.push_str("\n## Near-duplicate clusters\n\n");
    if near_dupes == 0 {
        s.push_str("No near-duplicates found at this threshold.\n");
    } else {
        for c in clusters.iter().filter(|c| c.members.len() > 1) {
            s.push_str(&format!("### {}\n\n", md_escape(&c.members[0].value)));
            s.push_str("| Value | Count |\n| --- | --- |\n");
            for (i, m) in c.members.iter().enumerate() {
                let val = if i == 0 {
                    format!("**{}**", md_escape(&m.value))
                } else {
                    md_escape(&m.value)
                };
                s.push_str(&format!("| {} | {} |\n", val, m.count));
            }
            s.push('\n');
        }
    }

    s.push_str("## Mapping\n\n");
    s.push_str("| Original | Canonical | Count |\n| --- | --- | --- |\n");
    for c in clusters {
        let canonical = md_escape(&c.members[0].value);
        for m in &c.members {
            s.push_str(&format!(
                "| {} | {} | {} |\n",
                md_escape(&m.value),
                canonical,
                m.count
            ));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_basics() {
        let a: Vec<char> = "kitten".chars().collect();
        let b: Vec<char> = "sitting".chars().collect();
        assert_eq!(levenshtein_bounded(&a, &b, 9), 3);
        // bound short-circuit still reports "over budget", not a wrong small value
        assert!(levenshtein_bounded(&a, &b, 1) > 1);
    }

    #[test]
    fn clusters_typos_casing_spacing() {
        // "New York" variants (typo, case, spacing) collapse into one cluster,
        // "Boston" stays on its own. JSON keeps the structure explicit.
        let data =
            "New York\nnew york\nNew  York\nNwe York\nBoston\nNew York\nNew York";
        let out = cluster(data, "", ",", false, 70.0, true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_values"], 7);
        assert_eq!(v["cluster_count"], 2);
        let clusters = v["clusters"].as_array().unwrap();
        // biggest cluster first, canonical = most frequent original
        assert_eq!(clusters[0]["canonical"], "New York");
        assert_eq!(clusters[0]["count"], 6);
        assert_eq!(clusters[0]["size"], 4);
        assert_eq!(clusters[1]["canonical"], "Boston");
    }

    #[test]
    fn casing_and_spacing_toggles_off_keep_variants_apart() {
        // With normalization off and a strict threshold, casing/spacing variants
        // do NOT merge.
        let data = "USA\nusa\nUSA";
        let out = cluster(data, "", ",", false, 100.0, false, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["cluster_count"], 2);
    }

    #[test]
    fn csv_column_by_name() {
        let data = "city,n\nParis,1\nParis,2\nparis,3\nBerlin,4";
        let out = cluster(data, "city", ",", true, 85.0, true, true, "csv").unwrap();
        assert_eq!(
            out,
            "cluster,original,canonical,count\n1,Paris,Paris,2\n1,paris,Paris,1\n2,Berlin,Berlin,1\n"
        );
    }

    #[test]
    fn markdown_has_mapping_and_canonical() {
        let data = "Apple\napple\napple\nBanana";
        let out = cluster(data, "", ",", false, 85.0, true, true, "markdown").unwrap();
        assert!(out.contains("# Value clusters"));
        assert!(out.contains("## Mapping"));
        // canonical is the most frequent original ("apple", count 2)
        assert!(out.contains("| Apple | apple |") || out.contains("| apple | apple |"));
        assert!(out.contains("### apple"));
    }

    #[test]
    fn errors() {
        assert!(cluster("  ", "", ",", false, 85.0, true, true, "markdown").is_err()); // empty
        assert!(cluster("a\nb", "", ",", false, 150.0, true, true, "json").is_err()); // bad threshold
        assert!(cluster("a\nb", "", ",", false, 85.0, true, true, "xml").is_err()); // bad output
        assert!(cluster("x,y\n1,2", "", ",", false, 85.0, true, true, "json").is_err()); // ambiguous column
        assert!(cluster("x,y\n1,2", "nope", ",", true, 85.0, true, true, "json").is_err()); // unknown name
        assert!(cluster("x,y\n1,2", "0", ",", false, 85.0, true, true, "json").is_err()); // 0 index
    }
}
