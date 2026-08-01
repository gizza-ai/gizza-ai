//! gizza-ai/duplicate-column-detector core — pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps.
//!
//! Detects **duplicate columns** in a table (CSV or any delimited text): two
//! columns are duplicates when they carry the **same sequence of values** down
//! every row. By default the comparison ignores the header *name*, so a column
//! called `email` and one called `email_addr` count as duplicates when their
//! cells match (the `df.T.drop_duplicates().T` idiom). Set `ignore_header_name`
//! off to also require the header names to match.
//!
//! Within each group of identical columns the **first (leftmost) column is
//! kept** and the rest are the redundant copies — mirroring pandas' keep-first
//! convention. The tool can either *report* the groups (`report`/`json`) or
//! *remove* the redundant columns and emit the de-duplicated table (`csv`).
//!
//! This is the column-oriented counterpart to `duplicate-row-finder` /
//! `csv-dedupe` (which work on rows). Optional case/whitespace normalization
//! catches columns that are identical apart from letter case or stray spacing.

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

/// Column label for reporting: `"name" (col N)` when a non-blank header exists,
/// else `col N` (1-based).
fn col_label(idx: usize, header: Option<&csv::StringRecord>) -> String {
    match header.and_then(|h| h.get(idx)) {
        Some(name) if !name.trim().is_empty() => format!("\"{}\" (col {})", name, idx + 1),
        _ => format!("col {}", idx + 1),
    }
}

/// Bare column name for structured output: the header value, or `column N`.
fn col_name(idx: usize, header: Option<&csv::StringRecord>) -> String {
    match header.and_then(|h| h.get(idx)) {
        Some(name) if !name.trim().is_empty() => name.to_string(),
        _ => format!("column {}", idx + 1),
    }
}

/// A group of columns that share the same (normalized) value sequence.
struct Group {
    /// Column indices, in original left-to-right order. `cols[0]` is kept.
    cols: Vec<usize>,
}

/// Detect duplicate columns and produce a report / de-duplicated CSV / JSON.
///
/// `output` is one of `report` (human summary), `csv` (the table with redundant
/// columns removed, first kept), or `json` (structured groups + summary).
///
/// When `ignore_header_name` is false and a header is present, columns must ALSO
/// share the same header name to count as duplicates; otherwise only the values
/// are compared (columns with different names but identical data still match).
#[allow(clippy::too_many_arguments)]
pub fn detect(
    data: &str,
    has_header: bool,
    delimiter: &str,
    ignore_case: bool,
    ignore_ws: bool,
    ignore_header_name: bool,
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
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    if width < 2 {
        return Err("need at least 2 columns to compare".into());
    }

    let header_owned = if has_header { records.first().cloned() } else { None };
    let header_ref = header_owned.as_ref();

    // Data rows only (skip the header row when present).
    let data_start = if has_header { 1 } else { 0 };
    let data_records = &records[data_start.min(records.len())..];
    if data_records.is_empty() {
        return Err("no data rows found (only a header)".into());
    }

    // --- Group columns by their normalized value sequence -------------------
    // The signature is the normalized cells joined top-to-bottom. When names must
    // match, the normalized header name is folded into the signature.
    let require_name = has_header && !ignore_header_name;
    let mut groups: Vec<Group> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for c in 0..width {
        let mut sig = String::new();
        if require_name {
            let name = header_ref.and_then(|h| h.get(c)).unwrap_or("");
            sig.push_str(&normalize(name, ignore_case, ignore_ws));
            sig.push('\u{2}'); // separate the name from the value stream
        }
        for rec in data_records {
            sig.push_str(&normalize(rec.get(c).unwrap_or(""), ignore_case, ignore_ws));
            sig.push('\u{1}');
        }
        if let Some(&gi) = index.get(&sig) {
            groups[gi].cols.push(c);
        } else {
            index.insert(sig, groups.len());
            groups.push(Group { cols: vec![c] });
        }
    }
    let dup_groups: Vec<&Group> = groups.iter().filter(|g| g.cols.len() > 1).collect();
    let redundant_cols: usize = dup_groups.iter().map(|g| g.cols.len() - 1).sum();
    let unique_cols = width - redundant_cols;
    // Indices to drop = every non-first column of each duplicate group.
    let drop_set: std::collections::HashSet<usize> = dup_groups
        .iter()
        .flat_map(|g| g.cols[1..].iter().copied())
        .collect();

    match out_mode {
        "json" => {
            use serde_json::json;
            let groups_json: Vec<_> = dup_groups
                .iter()
                .map(|g| {
                    json!({
                        "kept": { "column": col_name(g.cols[0], header_ref), "index": g.cols[0] + 1 },
                        "redundant": g.cols[1..]
                            .iter()
                            .map(|&c| json!({ "column": col_name(c, header_ref), "index": c + 1 }))
                            .collect::<Vec<_>>(),
                    })
                })
                .collect();
            let v = json!({
                "columns": width,
                "data_rows": data_records.len(),
                "ignore_header_name": !require_name,
                "duplicate_groups": dup_groups.len(),
                "redundant_columns": redundant_cols,
                "unique_columns": unique_cols,
                "groups": groups_json,
            });
            serde_json::to_string_pretty(&v).map_err(|e| format!("json error: {e}"))
        }
        "csv" => {
            // Emit every row (header included when present) with the redundant
            // columns removed, keeping the first of each duplicate group.
            let keep: Vec<usize> = (0..width).filter(|c| !drop_set.contains(c)).collect();
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(delim)
                .flexible(true)
                .from_writer(vec![]);
            for rec in &records {
                let row: Vec<&str> = keep.iter().map(|&c| rec.get(c).unwrap_or("")).collect();
                wtr.write_record(&row).map_err(|e| format!("CSV write error: {e}"))?;
            }
            let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
            String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
        }
        _ => {
            // report
            let mut out = String::new();
            out.push_str(&format!(
                "Scanned {} columns across {} data rows.\n",
                width,
                data_records.len()
            ));
            if dup_groups.is_empty() {
                out.push_str(
                    "No duplicate columns found — every column has a distinct set of values.\n",
                );
                return Ok(out);
            }
            out.push_str(&format!(
                "Found {} duplicate column group{}; {} redundant column{} can be removed ({} column{} remain unique).\n\n",
                dup_groups.len(),
                if dup_groups.len() == 1 { "" } else { "s" },
                redundant_cols,
                if redundant_cols == 1 { "" } else { "s" },
                unique_cols,
                if unique_cols == 1 { "" } else { "s" },
            ));
            out.push_str("Duplicate column groups (kept → redundant copies):\n");
            for g in &dup_groups {
                let dropped: Vec<String> =
                    g.cols[1..].iter().map(|&c| col_label(c, header_ref)).collect();
                out.push_str(&format!(
                    "  keep {}  ==  drop {}\n",
                    col_label(g.cols[0], header_ref),
                    dropped.join(", ")
                ));
            }
            out.push_str(
                "\nUse output=csv to get the table with the redundant columns removed.\n",
            );
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // id and id_copy are identical; name is unique; email == contact.
    const SAMPLE: &str =
        "id,name,email,id_copy,contact\n1,Alice,a@x.com,1,a@x.com\n2,Bob,b@y.com,2,b@y.com";

    #[test]
    fn report_finds_two_groups_keep_first() {
        let r = detect(SAMPLE, true, ",", true, true, true, "report").unwrap();
        assert!(
            r.contains("Found 2 duplicate column groups; 2 redundant columns can be removed (3 columns remain unique)."),
            "got: {r}"
        );
        assert!(r.contains("keep \"id\" (col 1)  ==  drop \"id_copy\" (col 4)"), "got: {r}");
        assert!(r.contains("keep \"email\" (col 3)  ==  drop \"contact\" (col 5)"), "got: {r}");
    }

    #[test]
    fn csv_removes_redundant_columns_first_kept() {
        let r = detect(SAMPLE, true, ",", true, true, true, "csv").unwrap();
        assert_eq!(r, "id,name,email\n1,Alice,a@x.com\n2,Bob,b@y.com\n");
    }

    #[test]
    fn json_reports_groups_and_summary() {
        let r = detect(SAMPLE, true, ",", true, true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["columns"], 5);
        assert_eq!(v["duplicate_groups"], 2);
        assert_eq!(v["redundant_columns"], 2);
        assert_eq!(v["unique_columns"], 3);
        let groups = v["groups"].as_array().unwrap();
        assert!(groups.iter().any(|g| g["kept"]["column"] == "id"
            && g["redundant"][0]["column"] == "id_copy"
            && g["redundant"][0]["index"] == 4));
    }

    #[test]
    fn ignore_header_name_toggle() {
        // email vs contact: identical values, different names.
        let d = "email,contact\na@x.com,a@x.com\nb@y.com,b@y.com";
        // Default (ignore name) → they are duplicates.
        let ignored = detect(d, true, ",", true, true, true, "report").unwrap();
        assert!(ignored.contains("Found 1 duplicate column group"), "ignored: {ignored}");
        // Require name match → different names, so NOT duplicates.
        let strict = detect(d, true, ",", true, true, false, "report").unwrap();
        assert!(strict.contains("No duplicate columns found"), "strict: {strict}");
    }

    #[test]
    fn same_name_and_values_match_when_name_required() {
        // Two columns literally named "x" with identical values.
        let d = "x,x\n1,1\n2,2";
        let strict = detect(d, true, ",", false, false, false, "report").unwrap();
        assert!(strict.contains("Found 1 duplicate column group"), "strict: {strict}");
    }

    #[test]
    fn normalization_toggles_near_identical_columns() {
        // "Alice"/"alice" and "NY"/"ny " differ only by case/space.
        let d = "a,b\nAlice,alice\nNY,ny ";
        let strict = detect(d, true, ",", false, false, true, "report").unwrap();
        assert!(strict.contains("No duplicate columns found"), "strict: {strict}");
        let norm = detect(d, true, ",", true, true, true, "report").unwrap();
        assert!(norm.contains("Found 1 duplicate column group"), "norm: {norm}");
    }

    #[test]
    fn no_header_uses_indices() {
        let d = "1,x,1\n2,y,2\n3,z,3"; // col1 == col3
        let r = detect(d, false, ",", true, true, true, "report").unwrap();
        assert!(r.contains("Scanned 3 columns across 3 data rows"), "got: {r}");
        assert!(r.contains("keep col 1  ==  drop col 3"), "got: {r}");
    }

    #[test]
    fn errors() {
        assert!(detect("   ", true, ",", true, true, true, "report").is_err()); // empty
        assert!(detect("a,b\n1,2", true, ",", true, true, true, "zzz").is_err()); // bad output
        assert!(detect("solo\n1\n2", true, ",", true, true, true, "report").is_err()); // 1 column
        assert!(detect("a,b", true, ",", true, true, true, "report").is_err()); // header only
    }
}
