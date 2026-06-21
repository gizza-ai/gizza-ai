//! gizza-ai/csv-find-incomplete-rows core — scan a CSV for malformed rows:
//! rows with too few or too many fields (relative to the expected width) and
//! rows with a blank cell in a column the caller marked as required. Pure-Rust
//! (`csv`); no wafer/wasm-bindgen deps. Shared by the chat block and the page.

use std::collections::HashSet;

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

/// One problem found on a data row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RowIssue {
    /// 1-based line number in the original CSV text (the header, if present,
    /// counts as line 1 — so the first data row of a headered CSV is line 2).
    pub line: usize,
    /// 1-based index of this row among data rows only (header excluded).
    pub row: usize,
    /// Number of fields the row actually has.
    pub fields: usize,
    /// One or more of: "too_few_fields", "too_many_fields", "blank_required".
    pub issues: Vec<String>,
    /// Required column names whose cell was blank on this row (empty unless a
    /// "blank_required" issue fired).
    pub blank_columns: Vec<String>,
    /// The row's raw cells, for context in the report.
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The expected number of fields per row (from the header, or the first row
    /// when there is no header).
    pub expected_fields: usize,
    /// Column names (from the header, or generated `col1`, `col2`, … names).
    pub columns: Vec<String>,
    /// Required column names that were actually checked (resolved from the
    /// caller's `required` spec).
    pub required_columns: Vec<String>,
    /// Total number of data rows scanned (header excluded).
    pub total_rows: usize,
    /// Number of data rows that had at least one issue.
    pub incomplete_rows: usize,
    /// The flagged rows, in document order.
    pub rows: Vec<RowIssue>,
}

/// Resolve the caller's `required` spec — a comma-separated list of column
/// names or 1-based column indices — into a set of 0-based column indices.
/// Unknown names / out-of-range indices are reported as an error so a typo
/// doesn't silently disable a check.
fn resolve_required(spec: &str, columns: &[String]) -> Result<Vec<usize>, String> {
    let mut out: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    for raw in spec.split(',') {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        let idx = if let Ok(n) = tok.parse::<usize>() {
            if n == 0 || n > columns.len() {
                return Err(format!(
                    "required column index {n} out of range (1..={})",
                    columns.len()
                ));
            }
            n - 1
        } else if let Some(i) = columns.iter().position(|c| c == tok) {
            i
        } else {
            return Err(format!("required column '{tok}' not found in header"));
        };
        if seen.insert(idx) {
            out.push(idx);
        }
    }
    Ok(out)
}

/// Scan `data` for incomplete rows.
///
/// - `has_header`: when true, the first row names the columns and sets the
///   expected field count; when false, the first row sets the expected count
///   and columns are named `col1`, `col2`, …
/// - `delimiter`: a single char or `comma`/`tab`/`semicolon`/`pipe`.
/// - `required`: comma-separated column names or 1-based indices that must be
///   non-blank on every data row (empty string = check none).
pub fn find_incomplete(
    data: &str,
    has_header: bool,
    delimiter: &str,
    required: &str,
) -> Result<Report, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true) // ragged rows are the whole point — don't error on them
        .from_reader(data.as_bytes());

    // Collect records together with their starting line number so the report
    // can point at the original text. A quoted field can span multiple lines,
    // so we read positions from the csv reader rather than counting '\n'.
    let mut records: Vec<(usize, csv::StringRecord)> = Vec::new();
    let mut rec = csv::StringRecord::new();
    loop {
        match rdr.read_record(&mut rec) {
            Ok(true) => {
                let line = rec.position().map(|p| p.line() as usize).unwrap_or(0);
                records.push((line, rec.clone()));
            }
            Ok(false) => break,
            Err(e) => return Err(format!("CSV parse error: {e}")),
        }
    }
    if records.is_empty() {
        return Err("no rows found".into());
    }

    // Expected width + column names come from the first row.
    let first = &records[0].1;
    let expected = first.len();
    let columns: Vec<String> = if has_header {
        (0..expected)
            .map(|i| {
                let n = first.get(i).unwrap_or("").trim();
                if n.is_empty() {
                    format!("col{}", i + 1)
                } else {
                    n.to_string()
                }
            })
            .collect()
    } else {
        (0..expected).map(|i| format!("col{}", i + 1)).collect()
    };

    let req_idx = resolve_required(required, &columns)?;
    let required_columns: Vec<String> = req_idx.iter().map(|&i| columns[i].clone()).collect();

    let data_records: &[(usize, csv::StringRecord)] =
        if has_header { &records[1..] } else { &records[..] };

    let mut rows: Vec<RowIssue> = Vec::new();
    for (di, (line, rec)) in data_records.iter().enumerate() {
        let mut issues: Vec<String> = Vec::new();
        if rec.len() < expected {
            issues.push("too_few_fields".to_string());
        } else if rec.len() > expected {
            issues.push("too_many_fields".to_string());
        }
        let mut blank_columns: Vec<String> = Vec::new();
        for &ci in &req_idx {
            // A field that's missing (short row) or present-but-blank both count.
            if rec.get(ci).map(|v| v.trim().is_empty()).unwrap_or(true) {
                blank_columns.push(columns[ci].clone());
            }
        }
        if !blank_columns.is_empty() {
            issues.push("blank_required".to_string());
        }
        if !issues.is_empty() {
            rows.push(RowIssue {
                line: *line,
                row: di + 1,
                fields: rec.len(),
                issues,
                blank_columns,
                values: rec.iter().map(|s| s.to_string()).collect(),
            });
        }
    }

    Ok(Report {
        expected_fields: expected,
        columns,
        required_columns,
        total_rows: data_records.len(),
        incomplete_rows: rows.len(),
        rows,
    })
}

/// Plain-text summary (used by the page) — one line per flagged row.
pub fn summary(
    data: &str,
    has_header: bool,
    delimiter: &str,
    required: &str,
) -> Result<String, String> {
    let r = find_incomplete(data, has_header, delimiter, required)?;
    let mut out = format!(
        "Expected {} field(s) per row. {} of {} data row(s) flagged.\n",
        r.expected_fields, r.incomplete_rows, r.total_rows
    );
    if r.rows.is_empty() {
        out.push_str("No incomplete rows found.");
        return Ok(out.trim_end().to_string());
    }
    for row in &r.rows {
        let mut detail = format!("{} ({} fields)", row.issues.join(", "), row.fields);
        if !row.blank_columns.is_empty() {
            detail.push_str(&format!(" [blank: {}]", row.blank_columns.join(", ")));
        }
        out.push_str(&format!("Line {} (data row {}): {}\n", row.line, row.row, detail));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_too_few_and_too_many() {
        let d = "a,b,c\n1,2,3\n4,5\n6,7,8,9";
        let r = find_incomplete(d, true, ",", "").unwrap();
        assert_eq!(r.expected_fields, 3);
        assert_eq!(r.total_rows, 3);
        assert_eq!(r.incomplete_rows, 2);
        // row 2 (line 3): too few
        assert_eq!(r.rows[0].row, 2);
        assert_eq!(r.rows[0].line, 3);
        assert_eq!(r.rows[0].issues, vec!["too_few_fields"]);
        // row 3 (line 4): too many
        assert_eq!(r.rows[1].row, 3);
        assert_eq!(r.rows[1].issues, vec!["too_many_fields"]);
    }

    #[test]
    fn complete_csv_has_no_issues() {
        let d = "a,b\n1,2\n3,4";
        let r = find_incomplete(d, true, ",", "").unwrap();
        assert_eq!(r.incomplete_rows, 0);
        assert!(r.rows.is_empty());
    }

    #[test]
    fn blank_required_by_name() {
        let d = "name,email\nAda,ada@x.com\nBo,\nCy,cy@x.com";
        let r = find_incomplete(d, true, ",", "email").unwrap();
        assert_eq!(r.required_columns, vec!["email"]);
        assert_eq!(r.incomplete_rows, 1);
        assert_eq!(r.rows[0].row, 2);
        assert_eq!(r.rows[0].issues, vec!["blank_required"]);
        assert_eq!(r.rows[0].blank_columns, vec!["email"]);
    }

    #[test]
    fn blank_required_by_index() {
        let d = "name,email\nAda,ada@x.com\n,bo@x.com";
        let r = find_incomplete(d, true, ",", "1").unwrap();
        assert_eq!(r.required_columns, vec!["name"]);
        assert_eq!(r.incomplete_rows, 1);
        assert_eq!(r.rows[0].blank_columns, vec!["name"]);
    }

    #[test]
    fn short_row_also_counts_as_blank_required() {
        // The required column is missing entirely on a short row.
        let d = "a,b,c\n1,2";
        let r = find_incomplete(d, true, ",", "c").unwrap();
        assert_eq!(r.incomplete_rows, 1);
        let issues = &r.rows[0].issues;
        assert!(issues.contains(&"too_few_fields".to_string()));
        assert!(issues.contains(&"blank_required".to_string()));
        assert_eq!(r.rows[0].blank_columns, vec!["c"]);
    }

    #[test]
    fn no_header_uses_first_row_width_and_col_names() {
        let d = "1,2,3\n4,5\n6,7,8";
        let r = find_incomplete(d, false, ",", "").unwrap();
        assert_eq!(r.expected_fields, 3);
        assert_eq!(r.columns, vec!["col1", "col2", "col3"]);
        assert_eq!(r.total_rows, 3); // first row is data, not a header
        assert_eq!(r.incomplete_rows, 1);
        assert_eq!(r.rows[0].row, 2);
    }

    #[test]
    fn whitespace_only_cell_is_blank() {
        let d = "name,note\nAda,   \nBo,ok";
        let r = find_incomplete(d, true, ",", "note").unwrap();
        assert_eq!(r.incomplete_rows, 1);
        assert_eq!(r.rows[0].blank_columns, vec!["note"]);
    }

    #[test]
    fn semicolon_delimiter() {
        let d = "a;b;c\n1;2;3\n4;5";
        let r = find_incomplete(d, true, ";", "").unwrap();
        assert_eq!(r.expected_fields, 3);
        assert_eq!(r.incomplete_rows, 1);
    }

    #[test]
    fn unknown_required_column_errors() {
        let d = "a,b\n1,2";
        let err = find_incomplete(d, true, ",", "nope").unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn out_of_range_required_index_errors() {
        let d = "a,b\n1,2";
        let err = find_incomplete(d, true, ",", "5").unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(find_incomplete("", true, ",", "").is_err());
        assert!(find_incomplete("a,b\n1,2", true, "xx", "").is_err());
    }

    #[test]
    fn summary_format() {
        let out = summary("a,b,c\n1,2,3\n4,5", true, ",", "").unwrap();
        assert!(out.contains("Expected 3 field(s)"));
        assert!(out.contains("1 of 2 data row(s) flagged"));
        assert!(out.contains("too_few_fields"));
    }

    #[test]
    fn summary_clean() {
        let out = summary("a,b\n1,2", true, ",", "").unwrap();
        assert!(out.contains("No incomplete rows found"));
    }

    #[test]
    fn multiple_required_columns() {
        let d = "name,email,phone\nAda,,123\nBo,bo@x.com,\nCy,cy@x.com,456";
        let r = find_incomplete(d, true, ",", "email,phone").unwrap();
        assert_eq!(r.required_columns, vec!["email", "phone"]);
        assert_eq!(r.incomplete_rows, 2);
        assert_eq!(r.rows[0].blank_columns, vec!["email"]);
        assert_eq!(r.rows[1].blank_columns, vec!["phone"]);
    }
}
