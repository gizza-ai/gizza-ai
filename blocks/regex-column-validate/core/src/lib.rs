//! regex-column-validate core — pure compute, shared by the chat skill block and the web page.
//!
//! Given CSV text, a target column, and a regular expression, flag the cells in
//! that column that do not match the pattern (or, in `matching` mode, the cells
//! that do). Reports total checked, valid count, invalid count, and a capped list
//! of the offending rows/values/messages. Pure-Rust (`csv` + `regex`); no
//! wafer/wasm-bindgen deps.

use regex::RegexBuilder;
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

/// One offending cell.
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

/// The full validation report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Resolved header name (or `col{n}`) of the validated column.
    pub column: String,
    /// 0-based index of the validated column.
    pub column_index: usize,
    /// The regular expression the values were checked against (as supplied).
    pub pattern: String,
    /// True when the pattern had to match the whole cell (anchored).
    pub full_match: bool,
    /// True when matching ignored ASCII/Unicode case.
    pub ignore_case: bool,
    /// Which cells are flagged: `non-matching` (default) or `matching`.
    pub report: String,
    /// Total data cells checked in the column (header excluded).
    pub total_checked: usize,
    /// Cells that passed (not flagged; plus blank cells when allow_blank is true).
    pub valid: usize,
    /// Cells that were flagged (invalid_rows may be truncated, this count is not).
    pub invalid: usize,
    /// True when invalid_rows was capped by max_issues.
    pub truncated: bool,
    /// The offending cells, in document order, capped at max_issues.
    pub invalid_rows: Vec<InvalidRow>,
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

/// Validate the chosen column against the regex. Returns the JSON or text report.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    column: &str,
    pattern: &str,
    full_match: bool,
    ignore_case: bool,
    report: &str,
    has_header: bool,
    allow_blank: bool,
    delimiter: &str,
    max_issues: usize,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if pattern.is_empty() {
        return Err("pattern is required (a regular expression to match against)".into());
    }
    let report_mode = match report.trim() {
        "" | "non-matching" => "non-matching",
        "matching" => "matching",
        other => {
            return Err(format!(
                "unknown report '{other}'; expected non-matching or matching"
            ))
        }
    };

    // Anchor the whole cell for full-match mode; otherwise match anywhere.
    let effective = if full_match {
        format!(r"\A(?:{pattern})\z")
    } else {
        pattern.to_string()
    };
    let re = RegexBuilder::new(&effective)
        .case_insensitive(ignore_case)
        .size_limit(1 << 22)
        .build()
        .map_err(|e| format!("invalid regex pattern: {e}"))?;

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
    let flag_matching = report_mode == "matching";
    let mut total_checked = 0usize;
    let mut valid = 0usize;
    let mut invalid = 0usize;
    let mut invalid_rows: Vec<InvalidRow> = Vec::new();

    for (di, (line, rec)) in data_records.iter().enumerate() {
        total_checked += 1;
        let raw = rec.get(col_idx);
        let cell = raw.unwrap_or("");
        let trimmed_empty = cell.trim().is_empty();

        let (is_valid, message) = if raw.is_none() {
            (false, "row has no value in the target column".to_string())
        } else if trimmed_empty {
            if allow_blank {
                (true, String::new())
            } else {
                (false, "blank value not allowed".to_string())
            }
        } else {
            let matched = re.is_match(cell);
            let flagged = matched == flag_matching;
            if flagged {
                let msg = if flag_matching {
                    "matches the pattern".to_string()
                } else {
                    "does not match the pattern".to_string()
                };
                (false, msg)
            } else {
                (true, String::new())
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
                    value: cell.to_string(),
                    message,
                });
            }
        }
    }

    let truncated = invalid > invalid_rows.len();

    let report = Report {
        column: columns[col_idx].clone(),
        column_index: col_idx,
        pattern: pattern.to_string(),
        full_match,
        ignore_case,
        report: report_mode.to_string(),
        total_checked,
        valid,
        invalid,
        truncated,
        invalid_rows,
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
        "Column \"{}\" (index {}) checked against /{}/\n",
        r.column, r.column_index, r.pattern
    ));
    out.push_str(&format!(
        "Match mode: {}, {}\n",
        if r.full_match { "full match" } else { "match anywhere" },
        if r.ignore_case { "case-insensitive" } else { "case-sensitive" },
    ));
    out.push_str(&format!(
        "Flagging: values that {} the pattern\n",
        if r.report == "matching" { "match" } else { "do not match" }
    ));
    out.push_str(&format!("Rows checked: {}\n", r.total_checked));
    out.push_str(&format!("Valid: {}\n", r.valid));
    out.push_str(&format!("Invalid: {}\n", r.invalid));

    if r.invalid == 0 {
        out.push_str(&format!("\nAll {} values are valid.", r.total_checked));
        return out;
    }

    out.push_str("\nInvalid values:\n");
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
            "… {} more invalid value(s) not shown (raise Max issues to list them).",
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

    const CSV: &str = "name,zip\nAda,12345\nBo,1234\nCy,90210";

    #[test]
    fn happy_all_valid() {
        let out = run(
            "id,zip\n1,12345\n2,90210", "zip", r"\d{5}", true, false, "non-matching", true, true,
            "auto", 50, "text",
        )
        .unwrap();
        assert!(out.contains("Valid: 2"), "{out}");
        assert!(out.contains("Invalid: 0"), "{out}");
        assert!(out.contains("All 2 values are valid."), "{out}");
    }

    #[test]
    fn detects_nonmatching_by_name() {
        let out = run(CSV, "zip", r"\d{5}", true, false, "non-matching", true, true, "auto", 50, "text").unwrap();
        assert!(out.contains("Rows checked: 3"), "{out}");
        assert!(out.contains("Valid: 2"), "{out}");
        assert!(out.contains("Invalid: 1"), "{out}");
        assert!(out.contains("row 2 (line 3): \"1234\" — does not match the pattern"), "{out}");
    }

    #[test]
    fn full_match_rejects_partial() {
        let out = run("v\n123456\n12345", "v", r"\d{5}", true, false, "non-matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // full match: 123456 is 6 digits, fails \A\d{5}\z; 12345 passes.
        assert_eq!(v["invalid"], 1);
        assert_eq!(v["invalid_rows"][0]["value"], "123456");
    }

    #[test]
    fn match_anywhere_allows_partial() {
        let out = run("v\n123456\nabc", "v", r"\d{5}", false, false, "non-matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // match anywhere: 123456 contains 5 digits → valid; abc → invalid.
        assert_eq!(v["valid"], 1);
        assert_eq!(v["invalid"], 1);
        assert_eq!(v["invalid_rows"][0]["value"], "abc");
    }

    #[test]
    fn ignore_case_flag() {
        let out = run("v\nHELLO\nhello\nWorld", "v", r"[a-z]+", true, true, "non-matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // case-insensitive full-match [a-z]+: all three are all-letters → match.
        assert_eq!(v["invalid"], 0);
    }

    #[test]
    fn case_sensitive_default() {
        let out = run("v\nHELLO\nhello", "v", r"[a-z]+", true, false, "non-matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 1);
        assert_eq!(v["invalid_rows"][0]["value"], "HELLO");
    }

    #[test]
    fn report_matching_flags_forbidden() {
        // Flag cells that DO match (find forbidden all-caps values).
        let out = run("v\nok\nFORBIDDEN\nfine", "v", r"[A-Z]+", true, false, "matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 1);
        assert_eq!(v["invalid_rows"][0]["value"], "FORBIDDEN");
        assert_eq!(v["invalid_rows"][0]["message"], "matches the pattern");
    }

    #[test]
    fn zero_based_index_headerless() {
        let out = run("12345\n99", "0", r"\d{5}", true, false, "non-matching", false, true, "auto", 50, "text").unwrap();
        assert!(out.contains("Rows checked: 2"), "{out}");
        assert!(out.contains("Invalid: 1"), "{out}");
    }

    #[test]
    fn inline_flags_in_pattern() {
        // Power users can embed inline flags; (?i) makes it case-insensitive here.
        let out = run("v\nHELLO", "v", r"(?i)hello", true, false, "non-matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 0);
    }

    #[test]
    fn blank_allowed_by_default() {
        let out = run("a,v\nx,\ny,12345", "v", r"\d{5}", true, false, "non-matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_checked"], 2);
        assert_eq!(v["invalid"], 0);
    }

    #[test]
    fn blank_cell_disallowed() {
        let out = run("a,v\nx,\ny,12345", "v", r"\d{5}", true, false, "non-matching", true, false, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 1);
        assert_eq!(v["invalid_rows"][0]["message"], "blank value not allowed");
    }

    #[test]
    fn max_issues_truncates() {
        let out = run("v\na\nb\nc", "v", r"\d+", true, false, "non-matching", true, true, "auto", 2, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 3);
        assert_eq!(v["truncated"], true);
        assert_eq!(v["invalid_rows"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn semicolon_delimiter_detected() {
        let out = run("a;v\n1;12345\n2;bad", "v", r"\d{5}", true, false, "non-matching", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 1);
    }

    // --- error paths ---

    #[test]
    fn empty_input_errors() {
        assert!(run("", "v", r"\d+", true, false, "non-matching", true, true, "auto", 50, "text").is_err());
    }

    #[test]
    fn empty_pattern_errors() {
        let err = run(CSV, "zip", "", true, false, "non-matching", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("pattern is required"), "{err}");
    }

    #[test]
    fn invalid_regex_errors() {
        let err = run(CSV, "zip", r"[unclosed", true, false, "non-matching", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("invalid regex pattern"), "{err}");
    }

    #[test]
    fn unknown_column_errors() {
        let err = run(CSV, "nope", r"\d+", true, false, "non-matching", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("not found in header"), "{err}");
    }

    #[test]
    fn index_out_of_range_errors() {
        let err = run("a,b\n1,2", "5", r"\d+", true, false, "non-matching", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn unknown_report_errors() {
        let err = run(CSV, "zip", r"\d+", true, false, "sideways", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("unknown report"), "{err}");
    }

    #[test]
    fn unknown_output_errors() {
        let err = run(CSV, "zip", r"\d+", true, false, "non-matching", true, true, "auto", 50, "xml").unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }
}
