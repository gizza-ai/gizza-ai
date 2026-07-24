//! gizza-ai/duplicate-row-finder core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Detects duplicate rows in a table (CSV or any delimited text) and *reports*
//! them — it does NOT rewrite or clean the data. A row is a duplicate when it
//! matches an earlier row on the chosen **key columns** (or the whole row when no
//! columns are given). Optional case/whitespace normalization catches
//! near-duplicates that differ only by letter case or spacing.
//!
//! Beyond the duplicate groups, it profiles **which columns drive the
//! duplication**: for every column it counts how many rows carry a value that is
//! repeated somewhere else, so you can tell whether the dupes come from (say) a
//! repeated `email` or a repeated `id`.
//!
//! This is the read-only counterpart to `csv-dedupe` (which removes exact dupes,
//! first kept) and `fuzzy-dedupe` (which removes typo-level near-dupes and emits
//! cleaned data). Here the output is a report, not a modified table.

use std::collections::HashMap;

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

/// Resolve `columns_csv` (1-based indices and/or header names) to 0-based column
/// indices. Empty → None (key on the whole row).
fn resolve_columns(
    columns_csv: &str,
    header: Option<&csv::StringRecord>,
) -> Result<Option<Vec<usize>>, String> {
    let toks: Vec<&str> = columns_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if toks.is_empty() {
        return Ok(None);
    }
    let mut idxs = Vec::new();
    for t in toks {
        if let Ok(n) = t.parse::<usize>() {
            if n == 0 {
                return Err("column indices are 1-based (>= 1)".into());
            }
            idxs.push(n - 1);
        } else if let Some(h) = header {
            match h.iter().position(|c| c == t) {
                Some(p) => idxs.push(p),
                None => return Err(format!("column '{t}' not found in the header")),
            }
        } else {
            return Err(format!(
                "column '{t}' is not a number and there is no header to match names"
            ));
        }
    }
    Ok(Some(idxs))
}

/// Normalize a cell for *comparison* (never for display).
fn normalize(v: &str, ignore_case: bool, ignore_ws: bool) -> String {
    let mut s = v.to_string();
    if ignore_ws {
        s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if ignore_case {
        s = s.to_lowercase();
    }
    s
}

/// A group of rows that share the same (normalized) key.
struct Group {
    /// First-seen display values for the key columns (original casing/spacing).
    display_key: Vec<String>,
    /// 1-based line numbers in the original text (header, if present, is line 1).
    lines: Vec<usize>,
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Column name for reporting: header value when available, else `column N` (1-based).
fn col_name(idx: usize, header: Option<&csv::StringRecord>) -> String {
    match header.and_then(|h| h.get(idx)) {
        Some(name) if !name.trim().is_empty() => name.to_string(),
        _ => format!("column {}", idx + 1),
    }
}

/// Find duplicate rows and produce a report.
///
/// `output` is one of `report` (human summary), `csv` (only the duplicate rows),
/// or `json` (structured groups + per-column analysis).
pub fn find(
    data: &str,
    columns: &str,
    has_header: bool,
    delimiter: &str,
    ignore_case: bool,
    ignore_ws: bool,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let out_mode = match output.trim() {
        "" | "report" => "report",
        "csv" => "csv",
        "json" => "json",
        other => return Err(format!("output must be report, csv or json, got '{other}'")),
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
        return Err("no rows found".into());
    }

    let header_owned = if has_header { records.first().cloned() } else { None };
    let header_ref = header_owned.as_ref();
    let key_cols = resolve_columns(columns, header_ref)?;

    // Data rows only (skip the header row when present).
    let data_start = if has_header { 1 } else { 0 };
    let data_records = &records[data_start.min(records.len())..];
    if data_records.is_empty() {
        return Err("no data rows found (only a header)".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    // --- Group rows by their normalized key ---------------------------------
    let key_idxs: Vec<usize> = match &key_cols {
        Some(cols) => cols.clone(),
        None => (0..width).collect(),
    };
    let mut groups: Vec<Group> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (di, rec) in data_records.iter().enumerate() {
        let line = data_start + di + 1; // 1-based line number in the original text
        let norm_key = key_idxs
            .iter()
            .map(|&c| normalize(rec.get(c).unwrap_or(""), ignore_case, ignore_ws))
            .collect::<Vec<_>>()
            .join("\u{1}");
        if let Some(&gi) = index.get(&norm_key) {
            groups[gi].lines.push(line);
        } else {
            let display_key = key_idxs
                .iter()
                .map(|&c| rec.get(c).unwrap_or("").to_string())
                .collect();
            index.insert(norm_key, groups.len());
            groups.push(Group { display_key, lines: vec![line] });
        }
    }
    let dup_groups: Vec<&Group> = groups.iter().filter(|g| g.lines.len() > 1).collect();
    let duplicate_rows: usize = dup_groups.iter().map(|g| g.lines.len()).sum();
    let unique_rows = data_records.len() - duplicate_rows;

    // --- Per-column duplication profile -------------------------------------
    // For each column, count rows whose (normalized, non-blank) value is repeated
    // in that column somewhere else — i.e. how much that column drives duplication.
    struct ColStat {
        idx: usize,
        name: String,
        dup_rows: usize,
        dup_values: usize,
        distinct: usize,
    }
    let mut col_stats: Vec<ColStat> = Vec::with_capacity(width);
    for c in 0..width {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for rec in data_records {
            let raw = rec.get(c).unwrap_or("");
            let n = normalize(raw, ignore_case, ignore_ws);
            if n.is_empty() {
                continue; // blank cells are not meaningful duplicates
            }
            *counts.entry(n).or_insert(0) += 1;
        }
        let distinct = counts.len();
        let dup_values = counts.values().filter(|&&v| v > 1).count();
        let dup_rows: usize = counts.values().filter(|&&v| v > 1).sum();
        col_stats.push(ColStat {
            idx: c,
            name: col_name(c, header_ref),
            dup_rows,
            dup_values,
            distinct,
        });
    }
    // Rank columns by how many rows they place into a repeat (the "drivers").
    col_stats.sort_by(|a, b| b.dup_rows.cmp(&a.dup_rows).then(a.idx.cmp(&b.idx)));

    let keyed_on: Vec<String> = match &key_cols {
        Some(cols) => cols.iter().map(|&c| col_name(c, header_ref)).collect(),
        None => vec![],
    };

    match out_mode {
        "json" => {
            use serde_json::json;
            let groups_json: Vec<_> = dup_groups
                .iter()
                .map(|g| {
                    json!({
                        "count": g.lines.len(),
                        "key": g.display_key,
                        "lines": g.lines,
                    })
                })
                .collect();
            let cols_json: Vec<_> = col_stats
                .iter()
                .map(|c| {
                    json!({
                        "column": c.name,
                        "index": c.idx + 1,
                        "duplicated_rows": c.dup_rows,
                        "duplicated_values": c.dup_values,
                        "distinct_values": c.distinct,
                    })
                })
                .collect();
            let v = json!({
                "data_rows": data_records.len(),
                "columns": width,
                "keyed_on": if keyed_on.is_empty() { json!("whole row") } else { json!(keyed_on) },
                "duplicate_groups": dup_groups.len(),
                "duplicate_rows": duplicate_rows,
                "unique_rows": unique_rows,
                "groups": groups_json,
                "column_analysis": cols_json,
            });
            serde_json::to_string_pretty(&v).map_err(|e| format!("json error: {e}"))
        }
        "csv" => {
            // Emit the header (if any) followed by only the rows that belong to a
            // duplicate group, in original order.
            let dup_set: std::collections::HashSet<usize> =
                dup_groups.iter().flat_map(|g| g.lines.iter().copied()).collect();
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(delim)
                .flexible(true)
                .from_writer(vec![]);
            if let Some(h) = header_ref {
                wtr.write_record(h).map_err(|e| format!("CSV write error: {e}"))?;
            }
            for (di, rec) in data_records.iter().enumerate() {
                let line = data_start + di + 1;
                if dup_set.contains(&line) {
                    wtr.write_record(rec).map_err(|e| format!("CSV write error: {e}"))?;
                }
            }
            let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
            String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
        }
        _ => {
            // report
            let mut out = String::new();
            let keyed_desc = if keyed_on.is_empty() {
                "the whole row".to_string()
            } else {
                keyed_on.join(", ")
            };
            out.push_str(&format!(
                "Scanned {} data rows ({} columns), keyed on {}.\n",
                data_records.len(),
                width,
                keyed_desc
            ));
            if dup_groups.is_empty() {
                out.push_str("No duplicate rows found — every row is unique on this key.\n");
                return Ok(out);
            }
            out.push_str(&format!(
                "Found {} duplicate group{} covering {} rows; {} row{} unique.\n\n",
                dup_groups.len(),
                if dup_groups.len() == 1 { "" } else { "s" },
                duplicate_rows,
                unique_rows,
                if unique_rows == 1 { " is" } else { "s are" },
            ));

            out.push_str("Duplicate groups (most repeated first):\n");
            // Sort groups by count desc, then first line asc, for a stable report.
            let mut sorted: Vec<&&Group> = dup_groups.iter().collect();
            sorted.sort_by(|a, b| {
                b.lines.len().cmp(&a.lines.len()).then(a.lines[0].cmp(&b.lines[0]))
            });
            for g in &sorted {
                let key_disp = truncate(&g.display_key.join(" | "), 80);
                let lines: Vec<String> = g.lines.iter().map(|l| l.to_string()).collect();
                out.push_str(&format!(
                    "  x{}  {}   (lines {})\n",
                    g.lines.len(),
                    key_disp,
                    lines.join(", ")
                ));
            }

            out.push_str("\nColumns driving duplication (repeated rows / distinct values):\n");
            for c in &col_stats {
                let tag = if c.dup_rows == 0 { "  (all unique)" } else { "" };
                out.push_str(&format!(
                    "  {}: {} / {}{}\n",
                    c.name, c.dup_rows, c.distinct, tag
                ));
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        "name,email\nAlice,a@x.com\nBob,b@y.com\nAlice,a@x.com\nCarol,c@z.com\nBob,b@y.com";

    #[test]
    fn report_finds_two_groups() {
        let r = find(SAMPLE, "", true, ",", false, false, "report").unwrap();
        assert!(r.contains("Found 2 duplicate groups covering 4 rows"), "got: {r}");
        assert!(r.contains("x2  Alice | a@x.com   (lines 2, 4)"), "got: {r}");
        assert!(r.contains("x2  Bob | b@y.com   (lines 3, 6)"), "got: {r}");
    }

    #[test]
    fn keyed_on_single_column() {
        // key on email only: Alice/Bob rows still group; Carol unique
        let r = find(SAMPLE, "email", true, ",", false, false, "report").unwrap();
        assert!(r.contains("keyed on email"), "got: {r}");
        assert!(r.contains("Found 2 duplicate groups"), "got: {r}");
    }

    #[test]
    fn near_duplicates_via_normalization() {
        // "ALICE" vs "Alice" and "a@x.com " (trailing space) collapse when normalized
        let d = "name,email\nAlice,a@x.com\nALICE, a@x.com ";
        let strict = find(d, "", true, ",", false, false, "report").unwrap();
        assert!(strict.contains("No duplicate rows found"), "strict: {strict}");
        let norm = find(d, "", true, ",", true, true, "report").unwrap();
        assert!(norm.contains("Found 1 duplicate group"), "norm: {norm}");
    }

    #[test]
    fn json_reports_column_analysis() {
        let r = find(SAMPLE, "", true, ",", false, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["duplicate_groups"], 2);
        assert_eq!(v["duplicate_rows"], 4);
        assert_eq!(v["unique_rows"], 1);
        // email drives duplication as much as name here (4 repeated rows each)
        let cols = v["column_analysis"].as_array().unwrap();
        assert!(cols.iter().any(|c| c["column"] == "email" && c["duplicated_rows"] == 4));
    }

    #[test]
    fn csv_output_emits_only_duplicate_rows() {
        let r = find(SAMPLE, "", true, ",", false, false, "csv").unwrap();
        assert_eq!(
            r,
            "name,email\nAlice,a@x.com\nBob,b@y.com\nAlice,a@x.com\nBob,b@y.com\n"
        );
    }

    #[test]
    fn no_header_indices_and_no_dupes() {
        let d = "x,1\ny,2\nz,3";
        let r = find(d, "1", false, ",", false, false, "report").unwrap();
        assert!(r.contains("No duplicate rows found"), "got: {r}");
    }

    #[test]
    fn errors() {
        assert!(find("   ", "", true, ",", false, false, "report").is_err()); // empty
        assert!(find("a,b\n1,2", "nope", true, ",", false, false, "report").is_err()); // bad col name
        assert!(find("a,b\n1,2", "0", true, ",", false, false, "report").is_err()); // 0 index
        assert!(find("a,b\n1,2", "", false, ",", false, false, "zzz").is_err()); // bad output
        assert!(find("name,email", "", true, ",", false, false, "report").is_err()); // header only
    }
}
