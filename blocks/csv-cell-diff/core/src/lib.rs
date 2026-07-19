//! csv-cell-diff core — column-aligned, cell-level diff of two CSVs.
//!
//! Aligns a `left` (old) and `right` (new) CSV **column-by-column** (by header
//! name when a header row is present, otherwise by position) and compares them
//! **cell-by-cell**, reporting every individual cell that differs plus which
//! rows were added or removed and which whole columns were added or removed.
//!
//! Rows are paired either by a **key column** (one or more, referenced by header
//! name or 1-based index) so reordered rows still match, or — when no key is
//! given — **positionally** by row order. Optional case- and whitespace-folding
//! affects only matching; the original cell text is always echoed in the output.
//!
//! Three renderers are provided: a readable `table` report, a structured `json`
//! report, and a flat `csv` change-log (one row per changed cell / added or
//! removed cell). Pure-Rust, no I/O — shared by the chat block and the web page.

use serde_json::json;
use std::collections::HashMap;

/// Comparison options that affect cell/key MATCHING only. The rendered text is
/// always the original, unmodified cell value.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
}

/// Map a delimiter name/char to its byte, mirroring the other csv-* blocks.
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
                    "delimiter must be a single char or one of comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// A parsed CSV side: column labels + the data rows (each a vector of cells).
struct Table {
    /// Column labels — header names (header=true) or `col1`, `col2`, … (header=false).
    cols: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Parse CSV text into a [`Table`]. With `header`, the first record supplies the
/// column labels; without it, labels are positional (`col1`, `col2`, …) sized to
/// the widest row. Ragged rows are allowed (short rows read `""` for missing
/// trailing cells).
fn parse(data: &str, delim: u8, header: bool, side: &str) -> Result<Table, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let recs: Vec<Vec<String>> = rdr
        .records()
        .map(|r| r.map(|rec| rec.iter().map(|c| c.to_string()).collect()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{side} CSV parse error: {e}"))?;
    if recs.is_empty() {
        return Err(format!("{side} CSV is empty"));
    }
    if header {
        let mut it = recs.into_iter();
        let cols = it.next().unwrap();
        if cols.is_empty() {
            return Err(format!("{side} CSV header row is empty"));
        }
        Ok(Table {
            cols,
            rows: it.collect(),
        })
    } else {
        let width = recs.iter().map(|r| r.len()).max().unwrap_or(0);
        let cols = (1..=width).map(|n| format!("col{n}")).collect();
        Ok(Table { cols, rows: recs })
    }
}

/// Resolve a key reference to a column index. A header NAME is matched first,
/// then a 1-based index. With `header=false` only indices are valid.
fn resolve_key(cols: &[String], key: &str, header: bool, side: &str) -> Result<usize, String> {
    let key = key.trim();
    if let Some(pos) = cols.iter().position(|c| c == key) {
        return Ok(pos);
    }
    if let Ok(n) = key.parse::<usize>() {
        if n >= 1 && n <= cols.len() {
            return Ok(n - 1);
        }
        return Err(format!(
            "{side} key index {n} out of range 1..={}",
            cols.len()
        ));
    }
    if header {
        Err(format!(
            "{side} key column '{key}' not found in header [{}]",
            cols.join(", ")
        ))
    } else {
        Err(format!(
            "{side} key '{key}' must be a 1-based column index when header=false"
        ))
    }
}

/// Cell at `idx`, or "" for a ragged (short) row.
fn cell(row: &[String], idx: usize) -> &str {
    row.get(idx).map(|s| s.as_str()).unwrap_or("")
}

/// Normalize a value for matching (never for display).
fn norm(v: &str, opt: &Options) -> String {
    let mut s = if opt.ignore_whitespace {
        v.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        v.to_string()
    };
    if opt.ignore_case {
        s = s.to_lowercase();
    }
    s
}

/// Build the composite match-key for a row from the resolved key column indices.
fn row_key(row: &[String], key_idx: &[usize], opt: &Options) -> String {
    key_idx
        .iter()
        .map(|&i| norm(cell(row, i), opt))
        .collect::<Vec<_>>()
        .join("\u{1}")
}

/// The original (un-normalized) key label shown to the user.
fn key_label(row: &[String], key_idx: &[usize]) -> String {
    key_idx
        .iter()
        .map(|&i| cell(row, i).to_string())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// One changed cell within a matched row.
struct CellChange {
    column: String,
    old: String,
    new: String,
}

/// A row's outcome in the diff.
enum RowDiff {
    /// Row present on both sides; `changes` lists every differing common cell.
    Changed { key: String, changes: Vec<CellChange> },
    /// Row present on both sides with no differing cell.
    Unchanged,
    /// Row only in the right CSV. Values are `(column, value)` for its columns.
    Added { key: String, values: Vec<(String, String)> },
    /// Row only in the left CSV.
    Removed { key: String, values: Vec<(String, String)> },
}

struct Diff {
    common_cols: Vec<String>,
    added_cols: Vec<String>,
    removed_cols: Vec<String>,
    rows: Vec<RowDiff>,
    rows_changed: usize,
    rows_added: usize,
    rows_removed: usize,
    rows_unchanged: usize,
    cells_changed: usize,
}

impl Diff {
    fn equal(&self) -> bool {
        self.added_cols.is_empty()
            && self.removed_cols.is_empty()
            && self.rows_changed == 0
            && self.rows_added == 0
            && self.rows_removed == 0
    }
}

/// Every `(column, value)` pair of a row, over the given column set.
fn row_values(cols: &[String], idx: &[usize], row: &[String]) -> Vec<(String, String)> {
    cols.iter()
        .zip(idx.iter())
        .map(|(c, &i)| (c.clone(), cell(row, i).to_string()))
        .collect()
}

/// Core diff: align columns, pair rows, compare common cells.
fn compute(
    left: &Table,
    right: &Table,
    key_cols: &[String],
    header: bool,
    opt: &Options,
) -> Result<Diff, String> {
    // Column alignment: common columns keep the LEFT order; a column present on
    // only one side is reported as added/removed (never compared cell-wise).
    let right_set: HashMap<&str, usize> = right
        .cols
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();
    let left_set: HashMap<&str, usize> = left
        .cols
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();

    let mut common_cols = Vec::new();
    let mut common_left_idx = Vec::new();
    let mut common_right_idx = Vec::new();
    for (i, c) in left.cols.iter().enumerate() {
        if let Some(&j) = right_set.get(c.as_str()) {
            common_cols.push(c.clone());
            common_left_idx.push(i);
            common_right_idx.push(j);
        }
    }
    let removed_cols: Vec<String> = left
        .cols
        .iter()
        .filter(|c| !right_set.contains_key(c.as_str()))
        .cloned()
        .collect();
    let added_cols: Vec<String> = right
        .cols
        .iter()
        .filter(|c| !left_set.contains_key(c.as_str()))
        .cloned()
        .collect();

    // Full column-index lists (for echoing whole added/removed rows).
    let left_all_idx: Vec<usize> = (0..left.cols.len()).collect();
    let right_all_idx: Vec<usize> = (0..right.cols.len()).collect();

    // Resolve key columns (present on BOTH sides) if any were requested.
    let left_key_idx: Vec<usize> = key_cols
        .iter()
        .map(|k| resolve_key(&left.cols, k, header, "left"))
        .collect::<Result<_, _>>()?;
    let right_key_idx: Vec<usize> = key_cols
        .iter()
        .map(|k| resolve_key(&right.cols, k, header, "right"))
        .collect::<Result<_, _>>()?;

    let mut rows: Vec<RowDiff> = Vec::new();
    let (mut rows_changed, mut rows_added, mut rows_removed, mut rows_unchanged, mut cells_changed) =
        (0usize, 0usize, 0usize, 0usize, 0usize);

    // Compare one matched (left,right) row pair, returning its RowDiff and the
    // count of changed cells.
    let compare_pair = |lrow: &[String], rrow: &[String], key: String| -> (RowDiff, usize) {
        let mut changes = Vec::new();
        for ((col, &li), &ri) in common_cols
            .iter()
            .zip(common_left_idx.iter())
            .zip(common_right_idx.iter())
        {
            let (lv, rv) = (cell(lrow, li), cell(rrow, ri));
            if norm(lv, opt) != norm(rv, opt) {
                changes.push(CellChange {
                    column: col.clone(),
                    old: lv.to_string(),
                    new: rv.to_string(),
                });
            }
        }
        if changes.is_empty() {
            (RowDiff::Unchanged, 0)
        } else {
            let n = changes.len();
            (RowDiff::Changed { key, changes }, n)
        }
    };

    if key_cols.is_empty() {
        // Positional pairing: row i of left vs row i of right.
        let n = left.rows.len().max(right.rows.len());
        for i in 0..n {
            match (left.rows.get(i), right.rows.get(i)) {
                (Some(l), Some(r)) => {
                    let (rd, cc) = compare_pair(l, r, format!("row {}", i + 1));
                    match &rd {
                        RowDiff::Unchanged => rows_unchanged += 1,
                        RowDiff::Changed { .. } => {
                            rows_changed += 1;
                            cells_changed += cc;
                        }
                        _ => {}
                    }
                    rows.push(rd);
                }
                (Some(l), None) => {
                    rows_removed += 1;
                    rows.push(RowDiff::Removed {
                        key: format!("row {}", i + 1),
                        values: row_values(&left.cols, &left_all_idx, l),
                    });
                }
                (None, Some(r)) => {
                    rows_added += 1;
                    rows.push(RowDiff::Added {
                        key: format!("row {}", i + 1),
                        values: row_values(&right.cols, &right_all_idx, r),
                    });
                }
                (None, None) => unreachable!(),
            }
        }
    } else {
        // Keyed pairing: match on the composite key; duplicates pair in order.
        let mut right_by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, r) in right.rows.iter().enumerate() {
            right_by_key
                .entry(row_key(r, &right_key_idx, opt))
                .or_default()
                .push(i);
        }
        // cursor into each key's right-index list.
        let mut cursor: HashMap<String, usize> = HashMap::new();
        let mut right_used = vec![false; right.rows.len()];

        for l in &left.rows {
            let k = row_key(l, &left_key_idx, opt);
            let c = cursor.entry(k.clone()).or_insert(0);
            let matched = right_by_key.get(&k).and_then(|v| v.get(*c).copied());
            if let Some(ri) = matched {
                *c += 1;
                right_used[ri] = true;
                let (rd, cc) = compare_pair(l, &right.rows[ri], key_label(l, &left_key_idx));
                match &rd {
                    RowDiff::Unchanged => rows_unchanged += 1,
                    RowDiff::Changed { .. } => {
                        rows_changed += 1;
                        cells_changed += cc;
                    }
                    _ => {}
                }
                rows.push(rd);
            } else {
                rows_removed += 1;
                rows.push(RowDiff::Removed {
                    key: key_label(l, &left_key_idx),
                    values: row_values(&left.cols, &left_all_idx, l),
                });
            }
        }
        for (i, r) in right.rows.iter().enumerate() {
            if !right_used[i] {
                rows_added += 1;
                rows.push(RowDiff::Added {
                    key: key_label(r, &right_key_idx),
                    values: row_values(&right.cols, &right_all_idx, r),
                });
            }
        }
    }

    Ok(Diff {
        common_cols,
        added_cols,
        removed_cols,
        rows,
        rows_changed,
        rows_added,
        rows_removed,
        rows_unchanged,
        cells_changed,
    })
}

/// Render the readable `table` report.
fn render_table(d: &Diff) -> String {
    if d.equal() {
        return "No differences.".to_string();
    }
    let mut out = String::new();
    // Column summary line.
    let mut cline = format!("{} columns compared", d.common_cols.len());
    if !d.added_cols.is_empty() {
        cline.push_str(&format!(
            " · {} column{} added ({})",
            d.added_cols.len(),
            if d.added_cols.len() == 1 { "" } else { "s" },
            d.added_cols.join(", ")
        ));
    }
    if !d.removed_cols.is_empty() {
        cline.push_str(&format!(
            " · {} column{} removed ({})",
            d.removed_cols.len(),
            if d.removed_cols.len() == 1 { "" } else { "s" },
            d.removed_cols.join(", ")
        ));
    }
    out.push_str(&cline);
    out.push('\n');
    // Row summary line.
    out.push_str(&format!(
        "{} rows changed · {} rows added · {} rows removed · {} rows unchanged · {} cells changed",
        d.rows_changed, d.rows_added, d.rows_removed, d.rows_unchanged, d.cells_changed
    ));

    let mut detail = String::new();
    for r in &d.rows {
        match r {
            RowDiff::Changed { key, changes } => {
                for c in changes {
                    detail.push_str(&format!(
                        "\n~ [{}] {}: \"{}\" → \"{}\"",
                        key, c.column, c.old, c.new
                    ));
                }
            }
            RowDiff::Added { key, values } => {
                detail.push_str(&format!("\n+ [{}] {}", key, join_kv(values)));
            }
            RowDiff::Removed { key, values } => {
                detail.push_str(&format!("\n- [{}] {}", key, join_kv(values)));
            }
            RowDiff::Unchanged => {}
        }
    }
    if !detail.is_empty() {
        out.push('\n');
        out.push_str(&detail);
    }
    out
}

fn join_kv(values: &[(String, String)]) -> String {
    values
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render the structured `json` report (pretty, 2-space). Unchanged rows are
/// omitted from `rows` (captured by the `rows_unchanged` count).
fn render_json(d: &Diff) -> String {
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for r in &d.rows {
        match r {
            RowDiff::Changed { key, changes } => {
                let cells: Vec<serde_json::Value> = changes
                    .iter()
                    .map(|c| json!({ "column": c.column, "old": c.old, "new": c.new }))
                    .collect();
                rows.push(json!({ "status": "changed", "key": key, "cells": cells }));
            }
            RowDiff::Added { key, values } => {
                rows.push(json!({ "status": "added", "key": key, "values": kv_object(values) }));
            }
            RowDiff::Removed { key, values } => {
                rows.push(json!({ "status": "removed", "key": key, "values": kv_object(values) }));
            }
            RowDiff::Unchanged => {}
        }
    }
    let report = json!({
        "equal": d.equal(),
        "columns": {
            "common": d.common_cols,
            "added": d.added_cols,
            "removed": d.removed_cols,
        },
        "summary": {
            "rows_changed": d.rows_changed,
            "rows_added": d.rows_added,
            "rows_removed": d.rows_removed,
            "rows_unchanged": d.rows_unchanged,
            "cells_changed": d.cells_changed,
        },
        "rows": rows,
    });
    serde_json::to_string_pretty(&report).unwrap()
}

fn kv_object(values: &[(String, String)]) -> serde_json::Map<String, serde_json::Value> {
    values
        .iter()
        .map(|(k, v)| (k.clone(), json!(v)))
        .collect()
}

/// Render the flat `csv` change-log: `row_key,status,column,old,new`. One line
/// per changed cell (status=changed), and one line per non-empty cell of each
/// added row (status=added, `new`=value) / removed row (status=removed,
/// `old`=value). Unchanged rows and whole added/removed columns are omitted.
fn render_csv(d: &Diff) -> String {
    let mut wtr = csv::WriterBuilder::new()
        .flexible(false)
        .from_writer(Vec::new());
    wtr.write_record(["row_key", "status", "column", "old", "new"])
        .unwrap();
    for r in &d.rows {
        match r {
            RowDiff::Changed { key, changes } => {
                for c in changes {
                    wtr.write_record([key, "changed", &c.column, &c.old, &c.new])
                        .unwrap();
                }
            }
            RowDiff::Added { key, values } => {
                for (col, v) in values {
                    if !v.is_empty() {
                        wtr.write_record([key, "added", col, "", v]).unwrap();
                    }
                }
            }
            RowDiff::Removed { key, values } => {
                for (col, v) in values {
                    if !v.is_empty() {
                        wtr.write_record([key, "removed", col, v, ""]).unwrap();
                    }
                }
            }
            RowDiff::Unchanged => {}
        }
    }
    let bytes = wtr.into_inner().unwrap();
    let mut s = String::from_utf8(bytes).unwrap();
    // Trim the single trailing newline for a clean one-string result.
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    s
}

/// Public entry used by the chat block, the CLI, and the web page.
///
/// - `key`: comma-separated key column names or 1-based indices to pair rows by;
///   empty ⇒ pair rows positionally.
/// - `delimiter`: `comma` | `tab` | `semicolon` | `pipe` (or a literal char).
/// - `header`: first row is a header (columns aligned by name); false ⇒ columns
///   aligned by position (`col1`, `col2`, …).
/// - `format`: `table` | `json` | `csv`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    key: &str,
    delimiter: &str,
    header: bool,
    ignore_case: bool,
    ignore_whitespace: bool,
    format: &str,
) -> Result<String, String> {
    let delim = delim_byte(delimiter)?;
    let opt = Options {
        ignore_case,
        ignore_whitespace,
    };
    let key_cols: Vec<String> = key
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let lt = parse(left, delim, header, "left")?;
    let rt = parse(right, delim, header, "right")?;
    let diff = compute(&lt, &rt, &key_cols, header, &opt)?;

    match format {
        "table" | "" => Ok(render_table(&diff)),
        "json" => Ok(render_json(&diff)),
        "csv" => Ok(render_csv(&diff)),
        other => Err(format!(
            "unknown format '{other}' — expected 'table', 'json', or 'csv'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_cell_change() {
        let left = "id,name,price\n1,Apple,10\n2,Banana,20";
        let right = "id,name,price\n1,Apple,12\n2,Banana,20";
        let out = run(left, right, "", "comma", true, false, false, "table").unwrap();
        assert!(out.contains("1 rows changed"), "{out}");
        assert!(out.contains("1 cells changed"), "{out}");
        assert!(out.contains("price: \"10\" → \"12\""), "{out}");
    }

    #[test]
    fn equal_inputs_report_no_diff() {
        let a = "id,name\n1,x\n2,y";
        let out = run(a, a, "", "comma", true, false, false, "table").unwrap();
        assert_eq!(out, "No differences.");
    }

    #[test]
    fn key_matching_ignores_row_order() {
        // Same data, rows reordered — with a key, no differences.
        let left = "id,name\n1,Alice\n2,Bob";
        let right = "id,name\n2,Bob\n1,Alice";
        let keyed = run(left, right, "id", "comma", true, false, false, "table").unwrap();
        assert_eq!(keyed, "No differences.", "keyed: {keyed}");
        // Positionally they look changed.
        let pos = run(left, right, "", "comma", true, false, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&pos).unwrap();
        assert_eq!(v["summary"]["rows_changed"], 2);
    }

    #[test]
    fn column_alignment_by_name_reordered() {
        // Right has columns in a different order — aligned by name, still equal.
        let left = "id,name,price\n1,Alice,10";
        let right = "price,id,name\n10,1,Alice";
        let out = run(left, right, "id", "comma", true, false, false, "table").unwrap();
        assert_eq!(out, "No differences.", "{out}");
    }

    #[test]
    fn added_and_removed_columns_flagged() {
        let left = "id,name,notes\n1,Alice,hi";
        let right = "id,name,stock\n1,Alice,5";
        let out = run(left, right, "id", "comma", true, false, false, "table").unwrap();
        assert!(out.contains("1 column added (stock)"), "{out}");
        assert!(out.contains("1 column removed (notes)"), "{out}");
    }

    #[test]
    fn added_and_removed_rows_by_key() {
        let left = "id,name\n1,Alice\n2,Bob";
        let right = "id,name\n1,Alice\n3,Carol";
        let out = run(left, right, "id", "comma", true, false, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["rows_added"], 1);
        assert_eq!(v["summary"]["rows_removed"], 1);
        assert_eq!(v["summary"]["rows_changed"], 0);
        assert_eq!(v["summary"]["rows_unchanged"], 1);
        // The added row is Carol, the removed row is Bob.
        let rows = v["rows"].as_array().unwrap();
        assert!(rows
            .iter()
            .any(|r| r["status"] == "added" && r["values"]["name"] == "Carol"));
        assert!(rows
            .iter()
            .any(|r| r["status"] == "removed" && r["values"]["name"] == "Bob"));
    }

    #[test]
    fn multi_key_composite() {
        let left = "first,last,age\nJan,Lee,30\nJan,Doe,40";
        let right = "first,last,age\nJan,Lee,31\nJan,Doe,40";
        let out = run(left, right, "first,last", "comma", true, false, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["rows_changed"], 1);
        assert_eq!(v["summary"]["cells_changed"], 1);
    }

    #[test]
    fn ignore_case_and_whitespace_matching() {
        let left = "id,name\n1,Hello  World";
        let right = "id,name\n1,hello world";
        let plain = run(left, right, "id", "comma", true, false, false, "table").unwrap();
        assert!(plain.contains("1 cells changed"), "{plain}");
        let folded = run(left, right, "id", "comma", true, true, true, "table").unwrap();
        assert_eq!(folded, "No differences.", "{folded}");
    }

    #[test]
    fn csv_change_log_format() {
        let left = "id,name,price\n1,Apple,10\n2,Banana,20";
        let right = "id,name,price\n1,Apple,12\n3,Cherry,30";
        let out = run(left, right, "id", "comma", true, false, false, "csv").unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(first, "row_key,status,column,old,new");
        assert!(out.contains("1,changed,price,10,12"), "{out}");
        // Removed row 2 (Banana) → one line per non-empty cell.
        assert!(out.contains("2,removed,name,Banana,"), "{out}");
        // Added row 3 (Cherry) → new value in the last column.
        assert!(out.contains("3,added,name,,Cherry"), "{out}");
    }

    #[test]
    fn no_header_positional_columns() {
        let left = "1,Apple,10\n2,Banana,20";
        let right = "1,Apple,12\n2,Banana,20";
        let out = run(left, right, "", "comma", false, false, false, "table").unwrap();
        // Columns are col1/col2/col3; the price column is col3.
        assert!(out.contains("col3: \"10\" → \"12\""), "{out}");
    }

    #[test]
    fn tab_delimiter() {
        let left = "id\tv\n1\ta";
        let right = "id\tv\n1\tb";
        let out = run(left, right, "id", "tab", true, false, false, "table").unwrap();
        assert!(out.contains("v: \"a\" → \"b\""), "{out}");
    }

    #[test]
    fn unknown_format_errors() {
        let e = run("a\n1", "a\n2", "", "comma", true, false, false, "html").unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }

    #[test]
    fn bad_delimiter_errors() {
        let e = run("a\n1", "a\n2", "", "nope", true, false, false, "table").unwrap_err();
        assert!(e.contains("delimiter must be"), "{e}");
    }

    #[test]
    fn missing_key_column_errors() {
        let e = run(
            "id,v\n1,a", "id,v\n1,a", "nope", "comma", true, false, false, "table",
        )
        .unwrap_err();
        assert!(e.contains("not found in header"), "{e}");
    }

    #[test]
    fn empty_input_errors() {
        let e = run("", "a\n1", "", "comma", true, false, false, "table").unwrap_err();
        assert!(e.contains("left CSV is empty"), "{e}");
    }

    #[test]
    fn quoted_fields_with_commas() {
        let left = "id,note\n1,\"a, b\"";
        let right = "id,note\n1,\"a, c\"";
        let out = run(left, right, "id", "comma", true, false, false, "table").unwrap();
        assert!(out.contains("note: \"a, b\" → \"a, c\""), "{out}");
    }
}
