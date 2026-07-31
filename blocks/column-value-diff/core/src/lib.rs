//! column-value-diff core — keyed single-value-column reconciliation of two tables.
//!
//! Joins a `left` (old) and `right` (new) CSV on one or more **key columns** and
//! compares a single chosen **value column**, reporting every key whose value
//! disagrees as a clean `key → old / new` pair. Unlike a full cell-by-cell diff,
//! this ignores every other column, so two files with different surrounding
//! schemas still reconcile cleanly on the one metric you care about (price, qty,
//! status, …).
//!
//! Rows are matched by the composite key (columns referenced by header name or
//! 1-based index); duplicate keys pair in order. Keys present on only one side
//! are reported as left-only / right-only when `include_unmatched` is on.
//! Optional case- and whitespace-folding affects MATCHING only — the original
//! text is always echoed. Three renderers: a readable `table` report, a
//! structured `json` report, and a flat `csv` change-log. Pure-Rust, no I/O.

use serde_json::json;
use std::collections::HashMap;

/// Comparison options that affect value/key MATCHING only. Rendered text is
/// always the original, unmodified value.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
    pub include_unmatched: bool,
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

/// Resolve a column reference to an index. A header NAME is matched first, then a
/// 1-based index. With `header=false` only indices are valid. `what` names the
/// field for a friendly error ("key" / "value").
fn resolve_col(
    cols: &[String],
    reference: &str,
    header: bool,
    side: &str,
    what: &str,
) -> Result<usize, String> {
    let reference = reference.trim();
    if let Some(pos) = cols.iter().position(|c| c == reference) {
        return Ok(pos);
    }
    if let Ok(n) = reference.parse::<usize>() {
        if n >= 1 && n <= cols.len() {
            return Ok(n - 1);
        }
        return Err(format!(
            "{side} {what} index {n} out of range 1..={}",
            cols.len()
        ));
    }
    if header {
        Err(format!(
            "{side} {what} column '{reference}' not found in header [{}]",
            cols.join(", ")
        ))
    } else {
        Err(format!(
            "{side} {what} '{reference}' must be a 1-based column index when header=false"
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

/// One matched key whose value column disagrees.
struct Change {
    key: String,
    old: String,
    new: String,
}

/// A key present on only one side (value echoed).
struct Unmatched {
    key: String,
    value: String,
}

struct Diff {
    value_col: String,
    changes: Vec<Change>,
    left_only: Vec<Unmatched>,
    right_only: Vec<Unmatched>,
    matched: usize,
    unchanged: usize,
    include_unmatched: bool,
}

impl Diff {
    /// Nothing worth reporting (respecting include_unmatched).
    fn empty(&self) -> bool {
        self.changes.is_empty()
            && (!self.include_unmatched || (self.left_only.is_empty() && self.right_only.is_empty()))
    }
}

/// Core reconciliation: join on the key columns, compare the one value column.
fn compute(
    left: &Table,
    right: &Table,
    key_cols: &[String],
    value_col: &str,
    header: bool,
    opt: &Options,
) -> Result<Diff, String> {
    // Resolve key + value columns on both sides.
    let left_key_idx: Vec<usize> = key_cols
        .iter()
        .map(|k| resolve_col(&left.cols, k, header, "left", "key"))
        .collect::<Result<_, _>>()?;
    let right_key_idx: Vec<usize> = key_cols
        .iter()
        .map(|k| resolve_col(&right.cols, k, header, "right", "key"))
        .collect::<Result<_, _>>()?;
    let left_val_idx = resolve_col(&left.cols, value_col, header, "left", "value")?;
    let right_val_idx = resolve_col(&right.cols, value_col, header, "right", "value")?;

    // Human label for the value column (header name, or the resolved col label).
    let value_label = left.cols[left_val_idx].clone();

    // Index right rows by key; duplicate keys keep insertion order.
    let mut right_by_key: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, r) in right.rows.iter().enumerate() {
        right_by_key
            .entry(row_key(r, &right_key_idx, opt))
            .or_default()
            .push(i);
    }
    let mut cursor: HashMap<String, usize> = HashMap::new();
    let mut right_used = vec![false; right.rows.len()];

    let mut changes = Vec::new();
    let mut left_only = Vec::new();
    let (mut matched, mut unchanged) = (0usize, 0usize);

    for l in &left.rows {
        let k = row_key(l, &left_key_idx, opt);
        let c = cursor.entry(k.clone()).or_insert(0);
        match right_by_key.get(&k).and_then(|v| v.get(*c).copied()) {
            Some(ri) => {
                *c += 1;
                right_used[ri] = true;
                matched += 1;
                let (lv, rv) = (cell(l, left_val_idx), cell(&right.rows[ri], right_val_idx));
                if norm(lv, opt) != norm(rv, opt) {
                    changes.push(Change {
                        key: key_label(l, &left_key_idx),
                        old: lv.to_string(),
                        new: rv.to_string(),
                    });
                } else {
                    unchanged += 1;
                }
            }
            None => left_only.push(Unmatched {
                key: key_label(l, &left_key_idx),
                value: cell(l, left_val_idx).to_string(),
            }),
        }
    }
    let mut right_only = Vec::new();
    for (i, r) in right.rows.iter().enumerate() {
        if !right_used[i] {
            right_only.push(Unmatched {
                key: key_label(r, &right_key_idx),
                value: cell(r, right_val_idx).to_string(),
            });
        }
    }

    let _ = value_col;
    Ok(Diff {
        value_col: value_label,
        changes,
        left_only,
        right_only,
        matched,
        unchanged,
        include_unmatched: opt.include_unmatched,
    })
}

/// Render the readable `table` report.
fn render_table(d: &Diff) -> String {
    if d.empty() {
        return "No differences.".to_string();
    }
    let mut out = String::new();
    let mut summary = format!(
        "value column \"{}\" · {} keys matched · {} changed · {} unchanged",
        d.value_col,
        d.matched,
        d.changes.len(),
        d.unchanged
    );
    if d.include_unmatched {
        summary.push_str(&format!(
            " · {} left-only · {} right-only",
            d.left_only.len(),
            d.right_only.len()
        ));
    }
    out.push_str(&summary);
    for c in &d.changes {
        out.push_str(&format!("\n~ [{}] \"{}\" → \"{}\"", c.key, c.old, c.new));
    }
    if d.include_unmatched {
        for u in &d.left_only {
            out.push_str(&format!("\n- [{}] left-only: \"{}\"", u.key, u.value));
        }
        for u in &d.right_only {
            out.push_str(&format!("\n+ [{}] right-only: \"{}\"", u.key, u.value));
        }
    }
    out
}

/// Render the structured `json` report (pretty, 2-space).
fn render_json(d: &Diff) -> String {
    let changes: Vec<serde_json::Value> = d
        .changes
        .iter()
        .map(|c| json!({ "key": c.key, "old": c.old, "new": c.new }))
        .collect();
    let mut summary = json!({
        "value_column": d.value_col,
        "matched": d.matched,
        "changed": d.changes.len(),
        "unchanged": d.unchanged,
    });
    let mut report = json!({
        "equal": d.empty(),
        "value_column": d.value_col,
        "changes": changes,
    });
    if d.include_unmatched {
        summary["left_only"] = json!(d.left_only.len());
        summary["right_only"] = json!(d.right_only.len());
        report["left_only"] = json!(d
            .left_only
            .iter()
            .map(|u| json!({ "key": u.key, "value": u.value }))
            .collect::<Vec<_>>());
        report["right_only"] = json!(d
            .right_only
            .iter()
            .map(|u| json!({ "key": u.key, "value": u.value }))
            .collect::<Vec<_>>());
    }
    report["summary"] = summary;
    serde_json::to_string_pretty(&report).unwrap()
}

/// Render the flat `csv` change-log: `key,status,old,new`. One line per changed
/// key; left-only / right-only lines only when `include_unmatched` is on.
fn render_csv(d: &Diff) -> String {
    let mut wtr = csv::WriterBuilder::new()
        .flexible(false)
        .from_writer(Vec::new());
    wtr.write_record(["key", "status", "old", "new"]).unwrap();
    for c in &d.changes {
        wtr.write_record([&c.key, &"changed".to_string(), &c.old, &c.new])
            .unwrap();
    }
    if d.include_unmatched {
        for u in &d.left_only {
            wtr.write_record([&u.key, &"left_only".to_string(), &u.value, &String::new()])
                .unwrap();
        }
        for u in &d.right_only {
            wtr.write_record([&u.key, &"right_only".to_string(), &String::new(), &u.value])
                .unwrap();
        }
    }
    let bytes = wtr.into_inner().unwrap();
    let mut s = String::from_utf8(bytes).unwrap();
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
/// - `key`: comma-separated key column names or 1-based indices to join rows by
///   (REQUIRED — this tool reconciles across a join key).
/// - `value`: the single value column (name or 1-based index) to compare.
/// - `delimiter`: `comma` | `tab` | `semicolon` | `pipe` (or a literal char).
/// - `header`: first row is a header (columns referenced by name); false ⇒
///   columns referenced by 1-based index (`col1`, `col2`, …).
/// - `ignore_case` / `ignore_whitespace`: fold when MATCHING keys and values.
/// - `include_unmatched`: also report keys present on only one side.
/// - `format`: `table` | `json` | `csv`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    key: &str,
    value: &str,
    delimiter: &str,
    header: bool,
    ignore_case: bool,
    ignore_whitespace: bool,
    include_unmatched: bool,
    format: &str,
) -> Result<String, String> {
    let delim = delim_byte(delimiter)?;
    let opt = Options {
        ignore_case,
        ignore_whitespace,
        include_unmatched,
    };
    let key_cols: Vec<String> = key
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if key_cols.is_empty() {
        return Err(
            "key is required — give the column(s) to join on (name or 1-based index), e.g. id"
                .into(),
        );
    }
    let value = value.trim();
    if value.is_empty() {
        return Err(
            "value is required — give the single column to compare (name or 1-based index), e.g. price"
                .into(),
        );
    }

    let lt = parse(left, delim, header, "left")?;
    let rt = parse(right, delim, header, "right")?;
    let diff = compute(&lt, &rt, &key_cols, value, header, &opt)?;

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
    fn value_change_reported_with_old_new() {
        let left = "id,name,price\n1,Apple,10\n2,Banana,20";
        let right = "id,name,price\n1,Apple,12\n2,Banana,20";
        let out = run(left, right, "id", "price", "comma", true, false, false, false, "table")
            .unwrap();
        assert!(out.contains("1 changed"), "{out}");
        assert!(out.contains("1 unchanged"), "{out}");
        assert!(out.contains("~ [1] \"10\" → \"12\""), "{out}");
    }

    #[test]
    fn other_columns_ignored() {
        // The name column differs but we only compare price — no differences.
        let left = "id,name,price\n1,Apple,10";
        let right = "id,name,price\n1,Apricot,10";
        let out = run(left, right, "id", "price", "comma", true, false, false, false, "table")
            .unwrap();
        assert_eq!(out, "No differences.", "{out}");
    }

    #[test]
    fn join_ignores_row_order() {
        let left = "id,price\n1,10\n2,20";
        let right = "id,price\n2,20\n1,11";
        let out = run(left, right, "id", "price", "comma", true, false, false, false, "json")
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["changes"][0]["key"], "1");
        assert_eq!(v["changes"][0]["old"], "10");
        assert_eq!(v["changes"][0]["new"], "11");
    }

    #[test]
    fn different_surrounding_schemas_still_reconcile() {
        // Left has notes, right has stock — irrelevant to the price reconciliation.
        let left = "id,notes,price\n1,hi,10";
        let right = "id,price,stock\n1,12,5";
        let out = run(left, right, "id", "price", "comma", true, false, false, false, "table")
            .unwrap();
        assert!(out.contains("~ [1] \"10\" → \"12\""), "{out}");
    }

    #[test]
    fn include_unmatched_reports_both_sides() {
        let left = "id,price\n1,10\n2,20";
        let right = "id,price\n1,10\n3,30";
        let out = run(left, right, "id", "price", "comma", true, false, false, true, "table")
            .unwrap();
        assert!(out.contains("1 left-only"), "{out}");
        assert!(out.contains("1 right-only"), "{out}");
        assert!(out.contains("- [2] left-only: \"20\""), "{out}");
        assert!(out.contains("+ [3] right-only: \"30\""), "{out}");
    }

    #[test]
    fn unmatched_hidden_by_default() {
        let left = "id,price\n1,10\n2,20";
        let right = "id,price\n1,10\n3,30";
        let out = run(left, right, "id", "price", "comma", true, false, false, false, "table")
            .unwrap();
        // No value change among matched keys, unmatched suppressed → clean.
        assert_eq!(out, "No differences.", "{out}");
    }

    #[test]
    fn composite_key() {
        let left = "first,last,age\nJan,Lee,30\nJan,Doe,40";
        let right = "first,last,age\nJan,Lee,31\nJan,Doe,40";
        let out = run(
            left, right, "first,last", "age", "comma", true, false, false, false, "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["changes"][0]["key"], "Jan | Lee");
    }

    #[test]
    fn ignore_case_and_whitespace() {
        let left = "id,status\n1,In Stock";
        let right = "id,status\n1,in  stock";
        let plain = run(left, right, "id", "status", "comma", true, false, false, false, "table")
            .unwrap();
        assert!(plain.contains("1 changed"), "{plain}");
        let folded = run(left, right, "id", "status", "comma", true, true, true, false, "table")
            .unwrap();
        assert_eq!(folded, "No differences.", "{folded}");
    }

    #[test]
    fn csv_change_log_format() {
        let left = "id,price\n1,10\n2,20";
        let right = "id,price\n1,12\n3,30";
        let out = run(left, right, "id", "price", "comma", true, false, false, true, "csv")
            .unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(first, "key,status,old,new");
        assert!(out.contains("1,changed,10,12"), "{out}");
        assert!(out.contains("2,left_only,20,"), "{out}");
        assert!(out.contains("3,right_only,,30"), "{out}");
    }

    #[test]
    fn json_summary_shape() {
        let left = "id,price\n1,10";
        let right = "id,price\n1,12";
        let out = run(left, right, "id", "price", "comma", true, false, false, false, "json")
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["equal"], false);
        assert_eq!(v["value_column"], "price");
        assert_eq!(v["summary"]["matched"], 1);
        // left_only/right_only omitted when include_unmatched is off.
        assert!(v["summary"].get("left_only").is_none());
        assert!(v.get("left_only").is_none());
    }

    #[test]
    fn value_by_index_no_header() {
        // No header: reference key/value by 1-based index. col1=id, col3=price.
        let left = "1,Apple,10\n2,Banana,20";
        let right = "1,Apple,12\n2,Banana,20";
        let out = run(left, right, "1", "3", "comma", false, false, false, false, "table")
            .unwrap();
        assert!(out.contains("~ [1] \"10\" → \"12\""), "{out}");
    }

    #[test]
    fn tab_delimiter() {
        let left = "id\tprice\n1\t10";
        let right = "id\tprice\n1\t11";
        let out = run(left, right, "id", "price", "tab", true, false, false, false, "table")
            .unwrap();
        assert!(out.contains("~ [1] \"10\" → \"11\""), "{out}");
    }

    #[test]
    fn quoted_value_with_comma() {
        let left = "id,note\n1,\"a, b\"";
        let right = "id,note\n1,\"a, c\"";
        let out = run(left, right, "id", "note", "comma", true, false, false, false, "table")
            .unwrap();
        assert!(out.contains("~ [1] \"a, b\" → \"a, c\""), "{out}");
    }

    #[test]
    fn missing_key_errors() {
        let e = run("id,price\n1,10", "id,price\n1,10", "", "price", "comma", true, false, false, false, "table")
            .unwrap_err();
        assert!(e.contains("key is required"), "{e}");
    }

    #[test]
    fn missing_value_errors() {
        let e = run("id,price\n1,10", "id,price\n1,10", "id", "", "comma", true, false, false, false, "table")
            .unwrap_err();
        assert!(e.contains("value is required"), "{e}");
    }

    #[test]
    fn unknown_value_column_errors() {
        let e = run("id,price\n1,10", "id,price\n1,10", "id", "cost", "comma", true, false, false, false, "table")
            .unwrap_err();
        assert!(e.contains("value column 'cost' not found"), "{e}");
    }

    #[test]
    fn missing_key_column_errors() {
        let e = run("id,price\n1,10", "id,price\n1,10", "nope", "price", "comma", true, false, false, false, "table")
            .unwrap_err();
        assert!(e.contains("key column 'nope' not found"), "{e}");
    }

    #[test]
    fn bad_delimiter_errors() {
        let e = run("id,price\n1,10", "id,price\n1,11", "id", "price", "nope", true, false, false, false, "table")
            .unwrap_err();
        assert!(e.contains("delimiter must be"), "{e}");
    }

    #[test]
    fn unknown_format_errors() {
        let e = run("id,price\n1,10", "id,price\n1,11", "id", "price", "comma", true, false, false, false, "html")
            .unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }

    #[test]
    fn empty_input_errors() {
        let e = run("", "id,price\n1,10", "id", "price", "comma", true, false, false, false, "table")
            .unwrap_err();
        assert!(e.contains("left CSV is empty"), "{e}");
    }
}
