//! category-canonicalize core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! **Applies a supplied mapping** to one (or several) categorical columns: every
//! spelling/case/whitespace variant listed in the mapping is rewritten to its
//! canonical value, and every value that is *not* covered gets a fuzzy
//! **suggestion** — the closest canonical from the supplied vocabulary, scored
//! 0–100 with a normalized Levenshtein ratio.
//!
//! The workflow it is built for is two passes: run once with the canonical
//! vocabulary you already know (`output = "suggestions"`) to see what is still
//! unmatched and what it probably should be, paste the accepted rows into the
//! mapping, run again to get the rewritten table.
//!
//! This is the *apply* side of the job. Discovering clusters with no supplied
//! vocabulary is a different tool (`cluster-similar-values`); here suggestions
//! are always scored against the canonicals YOU supplied, so the output is
//! reproducible and reviewable.

use std::collections::HashMap;

use serde::Serialize;

/// Hard cap on the pasted table, so a runaway paste can't wedge the tab.
pub const MAX_INPUT_BYTES: usize = 2_000_000;
/// Hard cap on the mapping text (a mapping is orders of magnitude smaller).
pub const MAX_MAPPING_BYTES: usize = 200_000;

/// Resolve a delimiter spec to its byte. `auto` sniffs the first non-empty line.
fn delim_byte(spec: &str, data: &str) -> Result<u8, String> {
    let s = spec.trim();
    Ok(match s {
        "auto" => sniff_delimiter(data),
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
                    "delimiter must be 'auto', a single character, or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Pick the delimiter that occurs most often outside quotes on the first
/// non-empty line. Ties (and a line with none of them) fall back to a comma.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut in_quote = false;
    let (mut comma, mut tab, mut semi, mut pipe) = (0usize, 0usize, 0usize, 0usize);
    for ch in line.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            _ if in_quote => {}
            ',' => comma += 1,
            '\t' => tab += 1,
            ';' => semi += 1,
            '|' => pipe += 1,
            _ => {}
        }
    }
    // Comma first so it wins every tie.
    [(b',', comma), (b'\t', tab), (b';', semi), (b'|', pipe)]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map(|(b, _)| b)
        .unwrap_or(b',')
}

/// What to do with a value the mapping doesn't cover.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Unmatched {
    /// Copy the original value through untouched (default).
    Keep,
    /// Replace with the best fuzzy suggestion when it scores at or above the
    /// threshold; otherwise keep the original.
    Fuzzy,
    /// Empty the cell.
    Blank,
    /// Fail with an error listing the offending values (strict pipelines).
    Error,
}

fn parse_unmatched(s: &str) -> Result<Unmatched, String> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Unmatched::Keep,
        "fuzzy" => Unmatched::Fuzzy,
        "blank" => Unmatched::Blank,
        "error" => Unmatched::Error,
        other => {
            return Err(format!(
                "unmatched must be keep, fuzzy, blank, or error, got '{other}'"
            ))
        }
    })
}

/// Fold a value into its comparison key. Case and spacing folding are the two
/// differences that dominate real categorical columns, so both are optional and
/// on by default.
fn key_of(value: &str, ignore_case: bool, ignore_spacing: bool) -> String {
    let mut s = if ignore_spacing {
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        value.to_string()
    };
    if ignore_case {
        s = s.to_lowercase();
    }
    s
}

/// The supplied vocabulary: variant key → canonical value, plus the canonical
/// values in the order they were declared (suggestion ties break toward the
/// first-declared canonical).
struct Mapping {
    by_key: HashMap<String, String>,
    canonicals: Vec<String>,
    canonical_keys: Vec<String>,
    rules: usize,
}

/// Split a mapping line into (variants, canonical). `None` = the line names a
/// canonical value with no variants of its own.
fn split_rule(line: &str) -> Option<(&str, &str)> {
    for sep in ["=>", "->"] {
        if let Some(i) = line.find(sep) {
            return Some((&line[..i], &line[i + sep.len()..]));
        }
    }
    for sep in ['=', '\t', ',', ';'] {
        if let Some(i) = line.find(sep) {
            return Some((&line[..i], &line[i + sep.len_utf8()..]));
        }
    }
    None
}

/// Parse the mapping text: one rule per line, `variant => canonical`
/// (`=>`, `->`, `=`, a tab, a comma, or a semicolon all separate). Several
/// variants share a canonical with `|`. A line with no separator declares a
/// canonical value that has no variants. `#` starts a comment line.
fn parse_mapping(
    mapping: &str,
    ignore_case: bool,
    ignore_spacing: bool,
) -> Result<Mapping, String> {
    if mapping.len() > MAX_MAPPING_BYTES {
        return Err(format!(
            "mapping is {} bytes, over the {MAX_MAPPING_BYTES}-byte limit",
            mapping.len()
        ));
    }
    let mut by_key: HashMap<String, String> = HashMap::new();
    let mut canonicals: Vec<String> = Vec::new();
    let mut seen_canonical: HashMap<String, ()> = HashMap::new();
    let mut variant_keys: Vec<(String, String)> = Vec::new();
    let mut rules = 0usize;

    for (n, raw_line) in mapping.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (variants, canonical) = match split_rule(line) {
            Some((lhs, rhs)) => (lhs.trim(), rhs.trim().to_string()),
            // No separator: the line IS a canonical value (an accepted value
            // with no listed variants).
            None => ("", line.to_string()),
        };
        if !canonical.is_empty() && seen_canonical.insert(canonical.clone(), ()).is_none() {
            canonicals.push(canonical.clone());
        }
        for variant in variants.split('|') {
            let variant = variant.trim();
            if variant.is_empty() {
                continue;
            }
            rules += 1;
            let key = key_of(variant, ignore_case, ignore_spacing);
            if let Some(prev) = by_key.get(&key) {
                if prev != &canonical {
                    return Err(format!(
                        "mapping line {}: '{variant}' is already mapped to '{prev}', cannot also map it to '{canonical}'",
                        n + 1
                    ));
                }
            }
            by_key.insert(key.clone(), canonical.clone());
            variant_keys.push((key, canonical.clone()));
        }
    }

    if by_key.is_empty() && canonicals.is_empty() {
        return Err("mapping is empty — supply at least one 'variant => canonical' rule (or a bare canonical value per line)".into());
    }

    // A value that is already canonical must match too. An explicit variant
    // rule wins over this implicit self-mapping.
    for c in &canonicals {
        let key = key_of(c, ignore_case, ignore_spacing);
        by_key.entry(key).or_insert_with(|| c.clone());
    }
    let canonical_keys = canonicals
        .iter()
        .map(|c| key_of(c, ignore_case, ignore_spacing))
        .collect();

    Ok(Mapping {
        by_key,
        canonicals,
        canonical_keys,
        rules,
    })
}

/// Levenshtein edit distance over chars (two-row DP — the values here are short
/// category labels, not documents).
fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Similarity of two comparison keys as a 0–100 ratio: `(1 − edits ÷ longer) × 100`.
fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 100.0;
    }
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let longer = ca.len().max(cb.len());
    if longer == 0 {
        return 100.0;
    }
    let d = levenshtein(&ca, &cb);
    (1.0 - d as f64 / longer as f64) * 100.0
}

/// Best canonical for an unmatched value, as (canonical, score).
fn best_suggestion(key: &str, m: &Mapping) -> Option<(String, f64)> {
    let mut best: Option<(usize, f64)> = None;
    for (i, ck) in m.canonical_keys.iter().enumerate() {
        let s = similarity(key, ck);
        if best.map(|(_, bs)| s > bs).unwrap_or(true) {
            best = Some((i, s));
        }
    }
    best.map(|(i, s)| (m.canonicals[i].clone(), (s * 10.0).round() / 10.0))
}

/// Resolve one `column` token (a 1-based index, or a header name when there is a
/// header row) to a 0-based index.
fn resolve_one(
    token: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<usize, String> {
    let t = token.trim();
    if let Some(h) = header {
        if let Some(i) = h.iter().position(|c| c.trim() == t) {
            return Ok(i);
        }
        if let Some(i) = h
            .iter()
            .position(|c| c.trim().eq_ignore_ascii_case(t))
        {
            return Ok(i);
        }
    }
    if let Ok(n) = t.parse::<usize>() {
        if n >= 1 && n <= width {
            return Ok(n - 1);
        }
        return Err(format!(
            "column '{t}' is out of range — the data has {width} column(s)"
        ));
    }
    match header {
        Some(h) => Err(format!(
            "column '{t}' is not in the header [{}]",
            h.iter().collect::<Vec<_>>().join(", ")
        )),
        None => Err(format!(
            "column '{t}' must be a 1-based index (1–{width}) — turn on 'first row is a header' to select by name"
        )),
    }
}

/// Resolve the whole `column` spec (comma-separated names/indexes; blank = the
/// only column) to 0-based indexes, in the order given, without duplicates.
fn resolve_columns(
    column: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<Vec<usize>, String> {
    let spec = column.trim();
    if spec.is_empty() {
        if width == 1 {
            return Ok(vec![0]);
        }
        return Err(format!(
            "column is required — the data has {width} columns, so name the one to canonicalize (a header name or a 1-based index)"
        ));
    }
    let mut out: Vec<usize> = Vec::new();
    for token in spec.split(',') {
        if token.trim().is_empty() {
            continue;
        }
        let i = resolve_one(token, header, width)?;
        if !out.contains(&i) {
            out.push(i);
        }
    }
    if out.is_empty() {
        return Err("column is empty — name at least one column".into());
    }
    Ok(out)
}

#[derive(Serialize)]
struct OutUnmatched {
    value: String,
    count: usize,
    suggestion: String,
    similarity: f64,
    applied: bool,
}

#[derive(Serialize)]
struct OutChange {
    from: String,
    to: String,
    count: usize,
}

#[derive(Serialize)]
struct Out {
    columns: Vec<String>,
    rows: usize,
    values_seen: usize,
    cells_changed: usize,
    matched_values: usize,
    unmatched_values: usize,
    blank_cells: usize,
    rules: usize,
    changes: Vec<OutChange>,
    unmatched: Vec<OutUnmatched>,
    csv: String,
}

/// Canonicalize categorical column(s) against a supplied mapping.
///
/// * `data` — a CSV/TSV table, or a plain list with one value per line.
/// * `mapping` — one rule per line: `variant => canonical`; `|` shares a
///   canonical between several variants; a line with no separator declares a
///   canonical value; `#` comments a line out.
/// * `column` — header name(s) or 1-based index/indexes, comma-separated; blank
///   uses the only column.
/// * `delimiter` — `auto`, a single character, or comma/tab/semicolon/pipe.
/// * `header` — treat the first row as a header (never rewritten).
/// * `ignore_case` / `ignore_spacing` — fold those differences when matching.
/// * `unmatched` — `keep` (default), `fuzzy`, `blank`, or `error`.
/// * `fuzzy_threshold` — 0–100; the score a suggestion needs before `fuzzy`
///   applies it.
/// * `output` — `csv` (the rewritten table), `markdown` (table + report),
///   `json`, or `suggestions` (a review CSV of the values still unmatched).
#[allow(clippy::too_many_arguments)]
pub fn canonicalize(
    data: &str,
    mapping: &str,
    column: &str,
    delimiter: &str,
    header: bool,
    ignore_case: bool,
    ignore_spacing: bool,
    unmatched: &str,
    fuzzy_threshold: f64,
    output: &str,
) -> Result<String, String> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !(0.0..=100.0).contains(&fuzzy_threshold) {
        return Err(format!(
            "fuzzy_threshold must be between 0 and 100, got {fuzzy_threshold}"
        ));
    }
    let policy = parse_unmatched(unmatched)?;
    let fmt = match output.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => "csv",
        "markdown" | "md" => "markdown",
        "json" => "json",
        "suggestions" => "suggestions",
        other => {
            return Err(format!(
                "output must be csv, markdown, json, or suggestions, got '{other}'"
            ))
        }
    };
    let map = parse_mapping(mapping, ignore_case, ignore_spacing)?;
    let delim = delim_byte(delimiter, data)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("input is empty".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(1);
    let cols = {
        let hdr = if header { records.first() } else { None };
        resolve_columns(column, hdr, width)?
    };
    let col_names: Vec<String> = cols
        .iter()
        .map(|&i| match if header { records.first() } else { None } {
            Some(h) => h.get(i).unwrap_or("").to_string(),
            None => format!("{}", i + 1),
        })
        .collect();

    // Pass 1: rewrite cells, tallying what happened.
    let mut values_seen = 0usize;
    let mut blank_cells = 0usize;
    let mut cells_changed = 0usize;
    let mut change_order: Vec<(String, String)> = Vec::new();
    let mut change_counts: HashMap<(String, String), usize> = HashMap::new();
    let mut miss_order: Vec<String> = Vec::new();
    let mut miss_counts: HashMap<String, usize> = HashMap::new();
    // key → (replacement, applied) for the unmatched policy, computed once.
    let mut miss_decision: HashMap<String, (String, bool)> = HashMap::new();

    for row in 0..records.len() {
        if header && row == 0 {
            continue;
        }
        let mut fields: Vec<String> = records[row].iter().map(|s| s.to_string()).collect();
        let mut touched = false;
        for &c in &cols {
            let Some(raw) = fields.get(c).cloned() else {
                continue;
            };
            if raw.trim().is_empty() {
                blank_cells += 1;
                continue;
            }
            values_seen += 1;
            let key = key_of(&raw, ignore_case, ignore_spacing);
            let new_value = match map.by_key.get(&key) {
                Some(canonical) => canonical.clone(),
                None => {
                    if !miss_counts.contains_key(&key) {
                        miss_order.push(raw.clone());
                    }
                    *miss_counts.entry(key.clone()).or_insert(0) += 1;
                    let decision = miss_decision.entry(key.clone()).or_insert_with(|| {
                        match policy {
                            Unmatched::Keep | Unmatched::Error => (raw.clone(), false),
                            Unmatched::Blank => (String::new(), true),
                            Unmatched::Fuzzy => match best_suggestion(&key, &map) {
                                Some((c, s)) if s >= fuzzy_threshold => (c, true),
                                _ => (raw.clone(), false),
                            },
                        }
                    });
                    decision.0.clone()
                }
            };
            if new_value != raw {
                cells_changed += 1;
                let pair = (raw.clone(), new_value.clone());
                if !change_counts.contains_key(&pair) {
                    change_order.push(pair.clone());
                }
                *change_counts.entry(pair).or_insert(0) += 1;
                fields[c] = new_value;
                touched = true;
            }
        }
        if touched {
            records[row] = csv::StringRecord::from(fields);
        }
    }

    // Distinct unmatched values, most frequent first, with their suggestion.
    let mut unmatched_out: Vec<OutUnmatched> = miss_order
        .iter()
        .map(|raw| {
            let key = key_of(raw, ignore_case, ignore_spacing);
            let count = *miss_counts.get(&key).unwrap_or(&0);
            let (suggestion, score) = best_suggestion(&key, &map)
                .map(|(c, s)| (c, s))
                .unwrap_or_else(|| (String::new(), 0.0));
            let applied = miss_decision
                .get(&key)
                .map(|(_, a)| *a)
                .unwrap_or(false)
                && policy == Unmatched::Fuzzy;
            OutUnmatched {
                value: raw.clone(),
                count,
                suggestion,
                similarity: score,
                applied,
            }
        })
        .collect();
    unmatched_out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| b.similarity.total_cmp(&a.similarity))
            .then_with(|| a.value.cmp(&b.value))
    });

    if policy == Unmatched::Error && !unmatched_out.is_empty() {
        let listed: Vec<String> = unmatched_out
            .iter()
            .take(10)
            .map(|u| format!("'{}'", u.value))
            .collect();
        let more = unmatched_out.len().saturating_sub(listed.len());
        return Err(format!(
            "{} value(s) are not covered by the mapping: {}{}",
            unmatched_out.len(),
            listed.join(", "),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        ));
    }

    let rows = records.len() - usize::from(header && !records.is_empty());
    let matched_values = values_seen - unmatched_out.iter().map(|u| u.count).sum::<usize>();

    let rendered = render_csv(&records, delim)?;
    let changes: Vec<OutChange> = change_order
        .iter()
        .map(|pair| OutChange {
            from: pair.0.clone(),
            to: pair.1.clone(),
            count: *change_counts.get(pair).unwrap_or(&0),
        })
        .collect();

    Ok(match fmt {
        "csv" => rendered,
        "suggestions" => render_suggestions(&unmatched_out, delim)?,
        "json" => {
            let out = Out {
                columns: col_names,
                rows,
                values_seen,
                cells_changed,
                matched_values,
                unmatched_values: unmatched_out.len(),
                blank_cells,
                rules: map.rules,
                changes,
                unmatched: unmatched_out,
                csv: rendered,
            };
            serde_json::to_string_pretty(&out).map_err(|e| format!("JSON error: {e}"))?
        }
        _ => render_markdown(
            &col_names,
            rows,
            values_seen,
            cells_changed,
            matched_values,
            &changes,
            &unmatched_out,
            fuzzy_threshold,
            policy,
            &rendered,
        ),
    })
}

/// Write records back out with the input's delimiter, quoting only where the
/// CSV grammar requires it.
fn render_csv(records: &[csv::StringRecord], delim: u8) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(csv::QuoteStyle::Necessary)
        .flexible(true)
        .from_writer(Vec::new());
    for rec in records {
        wtr.write_record(rec)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV write error: {e}"))
}

/// The review CSV: every value the mapping still doesn't cover, with the closest
/// canonical. Paste the rows you accept straight back into the mapping.
fn render_suggestions(unmatched: &[OutUnmatched], delim: u8) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(Vec::new());
    wtr.write_record(["value", "count", "suggestion", "similarity"])
        .map_err(|e| format!("CSV write error: {e}"))?;
    for u in unmatched {
        wtr.write_record([
            u.value.as_str(),
            &u.count.to_string(),
            u.suggestion.as_str(),
            &format!("{:.1}", u.similarity),
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV write error: {e}"))
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|")
}

#[allow(clippy::too_many_arguments)]
fn render_markdown(
    columns: &[String],
    rows: usize,
    values_seen: usize,
    cells_changed: usize,
    matched_values: usize,
    changes: &[OutChange],
    unmatched: &[OutUnmatched],
    threshold: f64,
    policy: Unmatched,
    rendered: &str,
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "## Canonicalized {} ({} row(s))\n\n",
        columns
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(", "),
        rows
    ));
    s.push_str(&format!(
        "- {values_seen} value(s) checked — {matched_values} matched the mapping, {} not covered\n",
        unmatched.len()
    ));
    s.push_str(&format!("- {cells_changed} cell(s) rewritten\n\n"));

    if changes.is_empty() {
        s.push_str("No cell needed rewriting — every value was already canonical.\n\n");
    } else {
        s.push_str("### Applied\n\n| From | To | Cells |\n| --- | --- | --- |\n");
        for c in changes {
            s.push_str(&format!(
                "| {} | {} | {} |\n",
                md_cell(&c.from),
                if c.to.is_empty() {
                    "*(blank)*".to_string()
                } else {
                    md_cell(&c.to)
                },
                c.count
            ));
        }
        s.push('\n');
    }

    if unmatched.is_empty() {
        s.push_str("### Not covered\n\nNone — the mapping covers every value.\n\n");
    } else {
        s.push_str(&format!(
            "### Not covered ({} value(s), closest canonical at threshold {threshold:.0})\n\n| Value | Count | Suggestion | Similarity |\n| --- | --- | --- | --- |\n",
            unmatched.len()
        ));
        for u in unmatched {
            s.push_str(&format!(
                "| {} | {} | {} | {:.1}{} |\n",
                md_cell(&u.value),
                u.count,
                md_cell(&u.suggestion),
                u.similarity,
                if u.applied { " ✓ applied" } else { "" }
            ));
        }
        if policy == Unmatched::Keep {
            s.push_str("\nThese were left as they are. Add the ones you accept to the mapping, or set *Unmatched values* to **fuzzy** to apply suggestions above the threshold.\n");
        }
        s.push('\n');
    }

    s.push_str("### Result\n\n```csv\n");
    s.push_str(rendered.trim_end_matches('\n'));
    s.push_str("\n```\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "USA|U.S.A.|us|united states => United States\nCanada|CAN => Canada";

    fn run_csv(data: &str, map: &str, column: &str) -> String {
        canonicalize(
            data, map, column, "auto", true, true, true, "keep", 85.0, "csv",
        )
        .unwrap()
    }

    #[test]
    fn happy_path_rewrites_variants_to_canonical() {
        let data = "country,n\nUSA,1\nu.s.a.,2\n  united   states ,3\nCAN,4\nCanada,5";
        let got = run_csv(data, MAP, "country");
        assert_eq!(
            got,
            "country,n\nUnited States,1\nUnited States,2\nUnited States,3\nCanada,4\nCanada,5\n"
        );
    }

    #[test]
    fn header_row_is_never_rewritten() {
        // A header cell that happens to be a mapped variant stays put.
        let data = "us,n\nUSA,1";
        let got = run_csv(data, MAP, "1");
        assert!(got.starts_with("us,n\n"), "got: {got}");
    }

    #[test]
    fn plain_newline_list_needs_no_column() {
        let got = canonicalize(
            "USA\nCAN\nMexico", MAP, "", "auto", false, true, true, "keep", 85.0, "csv",
        )
        .unwrap();
        assert_eq!(got, "United States\nCanada\nMexico\n");
    }

    #[test]
    fn unmatched_keep_leaves_the_value_alone() {
        let data = "country\nUSA\nBrazil";
        let got = run_csv(data, MAP, "country");
        assert_eq!(got, "country\nUnited States\nBrazil\n");
    }

    #[test]
    fn unmatched_fuzzy_applies_a_suggestion_above_the_threshold() {
        let data = "country\nCanadaa\nBrazil";
        let got = canonicalize(
            data, MAP, "country", "auto", true, true, true, "fuzzy", 80.0, "csv",
        )
        .unwrap();
        // "Canadaa" is 1 edit from "Canada" (85.7) → applied; Brazil is far → kept.
        assert_eq!(got, "country\nCanada\nBrazil\n");
    }

    #[test]
    fn unmatched_fuzzy_below_the_threshold_keeps_the_original() {
        let data = "country\nCanadaa";
        let got = canonicalize(
            data, MAP, "country", "auto", true, true, true, "fuzzy", 95.0, "csv",
        )
        .unwrap();
        assert_eq!(got, "country\nCanadaa\n");
    }

    #[test]
    fn unmatched_blank_empties_the_cell() {
        let data = "country,n\nUSA,1\nBrazil,2";
        let got = canonicalize(
            data, MAP, "country", "auto", true, true, true, "blank", 85.0, "csv",
        )
        .unwrap();
        assert_eq!(got, "country,n\nUnited States,1\n,2\n");
    }

    #[test]
    fn unmatched_error_reports_the_offenders() {
        let data = "country\nUSA\nBrazil\nPeru";
        let e = canonicalize(
            data, MAP, "country", "auto", true, true, true, "error", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("not covered by the mapping"), "got: {e}");
        assert!(e.contains("'Brazil'") && e.contains("'Peru'"), "got: {e}");
    }

    #[test]
    fn suggestions_output_is_a_review_csv() {
        let data = "country\nUSA\nCanadaa\nCanadaa\nUnited Sates";
        let got = canonicalize(
            data, MAP, "country", "auto", true, true, true, "keep", 85.0, "suggestions",
        )
        .unwrap();
        let lines: Vec<&str> = got.lines().collect();
        assert_eq!(lines[0], "value,count,suggestion,similarity");
        // Most frequent unmatched value first.
        assert_eq!(lines[1], "Canadaa,2,Canada,85.7");
        assert_eq!(lines[2], "United Sates,1,United States,92.3");
    }

    #[test]
    fn bare_lines_declare_canonicals_that_match_themselves() {
        let map = "United States\nCanada";
        let got = canonicalize(
            "country\nCanada\nUSA", map, "country", "auto", true, true, true, "keep", 85.0,
            "suggestions",
        )
        .unwrap();
        // Canada is already canonical (matched); USA is not covered — its closest
        // canonical is the (weak) "United States", well under any usable threshold.
        assert_eq!(got, "value,count,suggestion,similarity\nUSA,1,United States,23.1\n");
    }

    #[test]
    fn case_and_spacing_folding_can_be_turned_off() {
        let data = "country\nusa";
        let got = canonicalize(
            data, MAP, "country", "auto", true, false, true, "keep", 85.0, "csv",
        )
        .unwrap();
        // With ignore_case off, the mapping's lowercase `us` variant no longer
        // covers `usa` — but the literal `us` variant does not equal `usa`, so
        // the value is left alone.
        assert_eq!(got, "country\nusa\n");
    }

    #[test]
    fn several_columns_can_be_canonicalized_at_once() {
        let data = "from,to\nUSA,CAN\nCAN,us";
        let got = run_csv(data, MAP, "from,to");
        assert_eq!(
            got,
            "from,to\nUnited States,Canada\nCanada,United States\n"
        );
    }

    #[test]
    fn tab_delimited_input_round_trips() {
        let data = "country\tn\nUSA\t1";
        let got = run_csv(data, MAP, "country");
        assert_eq!(got, "country\tn\nUnited States\t1\n");
    }

    #[test]
    fn blank_cells_stay_blank() {
        let data = "country,n\n,1\nUSA,2";
        let got = run_csv(data, MAP, "country");
        assert_eq!(got, "country,n\n,1\nUnited States,2\n");
    }

    #[test]
    fn json_output_carries_stats_and_the_table() {
        let data = "country\nUSA\nBrazil";
        let got = canonicalize(
            data, MAP, "country", "auto", true, true, true, "keep", 85.0, "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(v["rows"], 2);
        assert_eq!(v["cells_changed"], 1);
        assert_eq!(v["unmatched_values"], 1);
        assert_eq!(v["unmatched"][0]["value"], "Brazil");
        assert_eq!(v["columns"][0], "country");
        assert!(v["csv"].as_str().unwrap().contains("United States"));
    }

    #[test]
    fn markdown_output_has_the_applied_and_not_covered_sections() {
        let data = "country\nUSA\nBrazil";
        let got = canonicalize(
            data, MAP, "country", "auto", true, true, true, "keep", 85.0, "markdown",
        )
        .unwrap();
        assert!(got.contains("### Applied"), "got: {got}");
        assert!(got.contains("| USA | United States | 1 |"), "got: {got}");
        assert!(got.contains("### Not covered"), "got: {got}");
        assert!(got.contains("```csv"), "got: {got}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let e = canonicalize(
            "   ", MAP, "", "auto", false, true, true, "keep", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("input is empty"), "got: {e}");
    }

    #[test]
    fn empty_mapping_is_an_error() {
        let e = canonicalize(
            "USA", "  \n# just a comment\n", "", "auto", false, true, true, "keep", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("mapping is empty"), "got: {e}");
    }

    #[test]
    fn conflicting_mapping_rules_are_an_error() {
        let e = canonicalize(
            "USA", "us => United States\nus => USA", "", "auto", false, true, true, "keep", 85.0,
            "csv",
        )
        .unwrap_err();
        assert!(e.contains("already mapped to"), "got: {e}");
    }

    #[test]
    fn a_missing_column_name_is_a_helpful_error() {
        let e = canonicalize(
            "country,n\nUSA,1", MAP, "nation", "auto", true, true, true, "keep", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("not in the header"), "got: {e}");
    }

    #[test]
    fn a_multi_column_table_needs_an_explicit_column() {
        let e = canonicalize(
            "USA,1\nCAN,2", MAP, "", "auto", false, true, true, "keep", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("column is required"), "got: {e}");
    }

    #[test]
    fn bad_enum_values_are_errors() {
        let e = canonicalize(
            "USA", MAP, "", "auto", false, true, true, "nope", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("unmatched must be"), "got: {e}");
        let e = canonicalize(
            "USA", MAP, "", "auto", false, true, true, "keep", 85.0, "xml",
        )
        .unwrap_err();
        assert!(e.contains("output must be"), "got: {e}");
        let e = canonicalize(
            "USA", MAP, "", "nope", false, true, true, "keep", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("delimiter must be"), "got: {e}");
        let e = canonicalize(
            "USA", MAP, "", "auto", false, true, true, "keep", 101.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("fuzzy_threshold must be"), "got: {e}");
    }

    #[test]
    fn input_at_the_cap_is_accepted_and_one_byte_over_is_rejected() {
        let row = "USA\n";
        let at_cap = row.repeat(MAX_INPUT_BYTES / row.len());
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(canonicalize(
            &at_cap, MAP, "", "auto", false, true, true, "keep", 85.0, "suggestions"
        )
        .is_ok());
        let over = format!("{at_cap}X");
        let e = canonicalize(
            &over, MAP, "", "auto", false, true, true, "keep", 85.0, "csv",
        )
        .unwrap_err();
        assert!(e.contains("over the"), "got: {e}");
    }

    #[test]
    fn similarity_is_a_normalized_edit_ratio() {
        assert_eq!(similarity("canada", "canada"), 100.0);
        assert!((similarity("canada", "canadaa") - 85.714).abs() < 0.01);
        assert_eq!(similarity("ab", "cd"), 0.0);
    }
}
