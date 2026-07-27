//! enum-domain-check core — pure compute, shared by the chat skill block and the web page.
//!
//! Given CSV text, a target column, and an allowed set of category values, flag
//! every cell in that column whose value is NOT one of the allowed categories (a
//! "domain check" / controlled-vocabulary / allowed-values membership check).
//! Reports total checked, valid count, invalid count, a capped list of the
//! offending rows, and a capped summary of the DISTINCT unexpected values with how
//! many times each occurred (so a single typo is easy to spot). Pure-Rust (`csv`);
//! no wafer/wasm-bindgen deps.

use serde::Serialize;

/// Map a delimiter name (or a literal single char) to its byte.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "auto" => 0, // sentinel: caller resolves via auto-detection
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be auto/comma/tab/semicolon/pipe or a single char, got '{other}'"
                ));
            }
        }
    })
}

/// Auto-detect the delimiter from the first non-blank physical line: pick the
/// candidate (comma, tab, semicolon, pipe) with the highest count, ties broken
/// by the listed preference order. Defaults to comma when nothing is found.
fn detect_delimiter(text: &str) -> u8 {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let candidates: [(u8, char); 4] = [(b',', ','), (b'\t', '\t'), (b';', ';'), (b'|', '|')];
    let mut best = b',';
    let mut best_n = 0usize;
    for (byte, ch) in candidates {
        let n = line.matches(ch).count();
        if n > best_n {
            best_n = n;
            best = byte;
        }
    }
    best
}

/// One offending cell (a value not in the allowed set).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InvalidRow {
    /// 1-based line number in the original CSV text (header is line 1 when present).
    pub line: usize,
    /// 1-based index of this row among DATA rows only (header excluded).
    pub row: usize,
    /// The offending cell value (empty string when the cell was blank or missing).
    pub value: String,
    /// Why it was flagged.
    pub message: String,
}

/// A distinct unexpected value and how many data cells held it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnexpectedValue {
    /// The unexpected value, exactly as it appeared (after trimming when `trim` is on).
    pub value: String,
    /// How many data cells in the column held this exact value.
    pub count: usize,
}

/// The full validation report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Resolved header name (or `col{n}`) of the validated column.
    pub column: String,
    /// 0-based index of the validated column.
    pub column_index: usize,
    /// The allowed category values, in the order supplied (deduped).
    pub allowed: Vec<String>,
    /// True when membership ignored ASCII/Unicode case.
    pub ignore_case: bool,
    /// True when each cell was trimmed of surrounding whitespace before comparison.
    pub trim: bool,
    /// Total data cells checked in the column (header excluded).
    pub total_checked: usize,
    /// Cells whose value is in the allowed set (plus blank cells when allow_blank is true).
    pub valid: usize,
    /// Cells that were flagged (invalid_rows may be truncated, this count is not).
    pub invalid: usize,
    /// True when invalid_rows was capped by max_issues.
    pub truncated: bool,
    /// The offending cells, in document order, capped at max_issues.
    pub invalid_rows: Vec<InvalidRow>,
    /// Distinct unexpected values with counts, most-frequent first, capped at max_issues.
    pub unexpected_values: Vec<UnexpectedValue>,
    /// True when unexpected_values was capped by max_issues.
    pub unexpected_truncated: bool,
}

/// Resolve the target column: a header name (when has_header), or a 0-based index.
/// A numeric `column` is always treated as a 0-based index; otherwise it is a header name.
fn resolve_column(column: &str, columns: &[String], has_header: bool) -> Result<usize, String> {
    let spec = column.trim();
    if spec.is_empty() {
        return Err("column is required (a header name or a 0-based column index)".into());
    }
    if let Ok(n) = spec.parse::<usize>() {
        if n >= columns.len() {
            return Err(format!(
                "column index {n} out of range (0..={})",
                columns.len().saturating_sub(1)
            ));
        }
        return Ok(n);
    }
    if !has_header {
        return Err(format!(
            "with 'First row is a header' off, column must be a 0-based index, got '{spec}'"
        ));
    }
    columns
        .iter()
        .position(|c| c == spec)
        .ok_or_else(|| format!("column '{spec}' not found in header ({})", columns.join(", ")))
}

/// Parse the comma-separated allowed set: trim each token, drop empties, dedupe
/// (preserving first-seen order). Returns an error when nothing usable remains.
fn parse_allowed(allowed: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for tok in allowed.split(',') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        if !out.iter().any(|e| e == t) {
            out.push(t.to_string());
        }
    }
    if out.is_empty() {
        return Err(
            "allowed set is empty — list the allowed category values, comma-separated (e.g. active,inactive,pending)"
                .into(),
        );
    }
    Ok(out)
}

/// Validate the chosen column against the allowed set. Returns the JSON or text report.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    column: &str,
    allowed: &str,
    ignore_case: bool,
    trim: bool,
    has_header: bool,
    allow_blank: bool,
    delimiter: &str,
    max_issues: usize,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let allowed_list = parse_allowed(allowed)?;
    // Membership keys: lowercased when ignore_case, so lookups are O(n) over a small set.
    let keys: Vec<String> = allowed_list
        .iter()
        .map(|v| if ignore_case { v.to_lowercase() } else { v.clone() })
        .collect();

    let delim = match delim_byte(delimiter)? {
        0 => detect_delimiter(data),
        b => b,
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

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

    // Column names come from the first row (header) or generated col{n} names.
    let width = records.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
    let first = &records[0].1;
    let columns: Vec<String> = if has_header {
        (0..width)
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
        (0..width).map(|i| format!("col{}", i + 1)).collect()
    };

    let col_idx = resolve_column(column, &columns, has_header)?;

    let data_records: &[(usize, csv::StringRecord)] =
        if has_header { &records[1..] } else { &records[..] };

    let cap = max_issues.max(1);
    let mut total_checked = 0usize;
    let mut valid = 0usize;
    let mut invalid = 0usize;
    let mut invalid_rows: Vec<InvalidRow> = Vec::new();
    // Distinct unexpected values with counts, in first-seen order.
    let mut unexpected: Vec<UnexpectedValue> = Vec::new();

    for (di, (line, rec)) in data_records.iter().enumerate() {
        total_checked += 1;
        let raw = rec.get(col_idx);
        let cell_owned = match raw {
            Some(c) if trim => c.trim().to_string(),
            Some(c) => c.to_string(),
            None => String::new(),
        };
        let is_blank = raw.is_none() || cell_owned.trim().is_empty();

        let (is_valid, message) = if raw.is_none() {
            if allow_blank {
                (true, String::new())
            } else {
                (false, "row has no value in the target column".to_string())
            }
        } else if is_blank {
            if allow_blank {
                (true, String::new())
            } else {
                (false, "blank value not allowed".to_string())
            }
        } else {
            let key = if ignore_case {
                cell_owned.to_lowercase()
            } else {
                cell_owned.clone()
            };
            if keys.iter().any(|k| *k == key) {
                (true, String::new())
            } else {
                (false, "not in the allowed set".to_string())
            }
        };

        if is_valid {
            valid += 1;
        } else {
            invalid += 1;
            if invalid_rows.len() < cap {
                invalid_rows.push(InvalidRow {
                    line: *line,
                    row: di + 1,
                    value: cell_owned.clone(),
                    message,
                });
            }
            // Tally the distinct unexpected value (skip blanks — they have no value).
            if !is_blank {
                if let Some(u) = unexpected.iter_mut().find(|u| u.value == cell_owned) {
                    u.count += 1;
                } else {
                    unexpected.push(UnexpectedValue {
                        value: cell_owned.clone(),
                        count: 1,
                    });
                }
            }
        }
    }

    let truncated = invalid > invalid_rows.len();

    // Most-frequent first, ties broken by first-seen order (stable sort by descending count).
    unexpected.sort_by(|a, b| b.count.cmp(&a.count));
    let unexpected_truncated = unexpected.len() > cap;
    unexpected.truncate(cap);

    let report = Report {
        column: columns[col_idx].clone(),
        column_index: col_idx,
        allowed: allowed_list,
        ignore_case,
        trim,
        total_checked,
        valid,
        invalid,
        truncated,
        invalid_rows,
        unexpected_values: unexpected,
        unexpected_truncated,
    };

    match output.trim() {
        "" | "text" => Ok(render_text(&report)),
        "json" => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        other => Err(format!("unknown output '{other}'; expected text or json")),
    }
}

fn render_text(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Column \"{}\" (index {}) checked against allowed set: {}\n",
        r.column,
        r.column_index,
        r.allowed.join(", ")
    ));
    out.push_str(&format!(
        "Matching: {}, {}\n",
        if r.ignore_case { "case-insensitive" } else { "case-sensitive" },
        if r.trim { "trimmed" } else { "untrimmed" },
    ));
    out.push_str(&format!("Rows checked: {}\n", r.total_checked));
    out.push_str(&format!("Valid: {}\n", r.valid));
    out.push_str(&format!("Invalid: {}\n", r.invalid));

    if r.invalid == 0 {
        out.push_str(&format!("\nAll {} values are in the allowed set.", r.total_checked));
        return out;
    }

    if !r.unexpected_values.is_empty() {
        out.push_str("\nUnexpected values:\n");
        for uv in &r.unexpected_values {
            out.push_str(&format!("  \"{}\" ×{}\n", uv.value, uv.count));
        }
        if r.unexpected_truncated {
            out.push_str("  … more distinct unexpected values not shown (raise Max issues).\n");
        }
    }

    out.push_str("\nInvalid cells:\n");
    for iv in &r.invalid_rows {
        if iv.value.is_empty() {
            out.push_str(&format!("  row {} (line {}): (blank) — {}\n", iv.row, iv.line, iv.message));
        } else {
            out.push_str(&format!(
                "  row {} (line {}): \"{}\" — {}\n",
                iv.row, iv.line, iv.value, iv.message
            ));
        }
    }
    if r.truncated {
        out.push_str(&format!(
            "… {} more invalid cell(s) not shown (raise Max issues to list them).",
            r.invalid - r.invalid_rows.len()
        ));
    } else {
        // Trim the trailing newline for a clean single block.
        while out.ends_with('\n') {
            out.pop();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "name,status\nAda,active\nBo,activ\nCy,inactive";

    #[test]
    fn happy_all_valid() {
        let out = run(
            "id,status\n1,active\n2,pending", "status", "active,inactive,pending",
            false, true, true, true, "auto", 50, "text",
        )
        .unwrap();
        assert!(out.contains("Valid: 2"), "{out}");
        assert!(out.contains("Invalid: 0"), "{out}");
        assert!(out.contains("All 2 values are in the allowed set."), "{out}");
    }

    #[test]
    fn detects_out_of_set_by_name() {
        let out = run(CSV, "status", "active,inactive", false, true, true, true, "auto", 50, "text").unwrap();
        assert!(out.contains("Rows checked: 3"), "{out}");
        assert!(out.contains("Valid: 2"), "{out}");
        assert!(out.contains("Invalid: 1"), "{out}");
        assert!(out.contains("row 2 (line 3): \"activ\" — not in the allowed set"), "{out}");
    }

    #[test]
    fn unexpected_values_counts() {
        let out = run(
            "v\nactiv\nactiv\nActive\nactive", "v", "active",
            false, true, true, true, "auto", 50, "json",
        )
        .unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        // case-sensitive: "activ" ×2 and "Active" ×1 are unexpected; "active" ×1 is valid.
        assert_eq!(j["invalid"], 3);
        assert_eq!(j["valid"], 1);
        assert_eq!(j["unexpected_values"][0]["value"], "activ");
        assert_eq!(j["unexpected_values"][0]["count"], 2);
        assert_eq!(j["unexpected_values"][1]["value"], "Active");
        assert_eq!(j["unexpected_values"][1]["count"], 1);
    }

    #[test]
    fn ignore_case_membership() {
        let out = run("v\nACTIVE\nActive\nactive", "v", "active", true, true, true, true, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["invalid"], 0);
    }

    #[test]
    fn case_sensitive_default() {
        let out = run("v\nACTIVE\nactive", "v", "active", false, true, true, true, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["invalid"], 1);
        assert_eq!(j["invalid_rows"][0]["value"], "ACTIVE");
    }

    #[test]
    fn trim_on_by_default() {
        // "  active  " trims to a valid value.
        let out = run("v\n  active  \ninactive", "v", "active,inactive", false, true, true, true, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["invalid"], 0);
    }

    #[test]
    fn trim_off_flags_padded_value() {
        let out = run("v\n  active", "v", "active", false, false, true, true, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["invalid"], 1);
        // Untrimmed value retains its padding.
        assert_eq!(j["invalid_rows"][0]["value"], "  active");
    }

    #[test]
    fn zero_based_index_headerless() {
        let out = run("active\nnope", "0", "active", false, true, false, true, "auto", 50, "text").unwrap();
        assert!(out.contains("Rows checked: 2"), "{out}");
        assert!(out.contains("Invalid: 1"), "{out}");
    }

    #[test]
    fn blank_allowed_by_default() {
        let out = run("a,v\nx,\ny,active", "v", "active", false, true, true, true, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["total_checked"], 2);
        assert_eq!(j["invalid"], 0);
    }

    #[test]
    fn blank_cell_disallowed() {
        let out = run("a,v\nx,\ny,active", "v", "active", false, true, true, false, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["invalid"], 1);
        assert_eq!(j["invalid_rows"][0]["message"], "blank value not allowed");
        // A blank does not become a distinct unexpected value.
        assert_eq!(j["unexpected_values"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn allowed_set_dedupes_and_trims() {
        let out = run("v\nactive", "v", " active , active ,inactive", false, true, true, true, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["allowed"].as_array().unwrap().len(), 2);
        assert_eq!(j["allowed"][0], "active");
        assert_eq!(j["allowed"][1], "inactive");
    }

    #[test]
    fn max_issues_truncates_rows() {
        let out = run("v\nx\nx\ny\nz", "v", "active", false, true, true, true, "auto", 2, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["invalid"], 4);
        assert_eq!(j["truncated"], true);
        assert_eq!(j["invalid_rows"].as_array().unwrap().len(), 2);
        // Distinct unexpected values (x,y,z = 3) are also capped at max_issues=2.
        assert_eq!(j["unexpected_values"].as_array().unwrap().len(), 2);
        assert_eq!(j["unexpected_truncated"], true);
        assert_eq!(j["unexpected_values"][0]["value"], "x");
        assert_eq!(j["unexpected_values"][0]["count"], 2);
    }

    #[test]
    fn semicolon_delimiter_detected() {
        let out = run("a;status\n1;active\n2;bad", "status", "active", false, true, true, true, "auto", 50, "json").unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["invalid"], 1);
    }

    // --- error paths ---

    #[test]
    fn empty_input_errors() {
        assert!(run("", "v", "active", false, true, true, true, "auto", 50, "text").is_err());
    }

    #[test]
    fn empty_allowed_errors() {
        let err = run(CSV, "status", "  ,  ", false, true, true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("allowed set is empty"), "{err}");
    }

    #[test]
    fn unknown_column_errors() {
        let err = run(CSV, "nope", "active", false, true, true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("not found in header"), "{err}");
    }

    #[test]
    fn index_out_of_range_errors() {
        let err = run("a,b\n1,2", "5", "active", false, true, true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn unknown_output_errors() {
        let err = run(CSV, "status", "active", false, true, true, true, "auto", 50, "xml").unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn bad_delimiter_errors() {
        let err = run(CSV, "status", "active", false, true, true, true, "nope", 50, "text").unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }
}
