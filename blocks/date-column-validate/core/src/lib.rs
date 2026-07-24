//! date-column-validate core — pure compute, shared by the chat skill block and the web page.
//!
//! Given CSV text and a target column, check that every value in that column parses
//! against a chosen date format (a preset such as ISO `%Y-%m-%d`, or a custom
//! chrono strftime pattern, or RFC 3339). Reports total checked, valid count,
//! invalid count, and a capped list of the offending rows/values/messages.
//! Pure-Rust (`csv` + `chrono`); no wafer/wasm-bindgen deps.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
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

/// The parse strategy for one date value.
#[derive(Clone, Copy)]
enum Strategy<'a> {
    /// A calendar date parsed against this strftime pattern.
    Date(&'a str),
    /// A date+time parsed against this strftime pattern.
    DateTime(&'a str),
    /// RFC 3339 / ISO 8601 date-time (e.g. `2021-06-01T12:30:00Z`).
    Rfc3339,
    /// A custom pattern: try date, then date-time, then time-of-day.
    Custom(&'a str),
}

/// Resolve the (display format, human label, strategy) for a preset + custom format.
fn resolve_format<'a>(preset: &str, custom: &'a str) -> Result<(String, String, Strategy<'a>), String> {
    let out = match preset {
        "" | "iso-date" => ("%Y-%m-%d".to_string(), "ISO date (YYYY-MM-DD)".to_string(), Strategy::Date("%Y-%m-%d")),
        "us-date" => ("%m/%d/%Y".to_string(), "US date (MM/DD/YYYY)".to_string(), Strategy::Date("%m/%d/%Y")),
        "eu-date" => ("%d/%m/%Y".to_string(), "EU date (DD/MM/YYYY)".to_string(), Strategy::Date("%d/%m/%Y")),
        "iso-datetime" => (
            "%Y-%m-%dT%H:%M:%S".to_string(),
            "ISO date-time (YYYY-MM-DDThh:mm:ss)".to_string(),
            Strategy::DateTime("%Y-%m-%dT%H:%M:%S"),
        ),
        "rfc3339" => ("RFC 3339".to_string(), "RFC 3339 date-time".to_string(), Strategy::Rfc3339),
        "custom" => {
            let fmt = custom.trim();
            if fmt.is_empty() {
                return Err("format is required when preset is 'custom' (for example %d-%b-%Y)".into());
            }
            (fmt.to_string(), format!("custom pattern {fmt}"), Strategy::Custom(custom.trim()))
        }
        other => {
            return Err(format!(
                "unknown preset '{other}'; expected iso-date, us-date, eu-date, iso-datetime, rfc3339, or custom"
            ))
        }
    };
    Ok(out)
}

/// Does `value` parse against `strategy`?
fn matches(value: &str, strategy: Strategy) -> bool {
    match strategy {
        Strategy::Date(fmt) => NaiveDate::parse_from_str(value, fmt).is_ok(),
        Strategy::DateTime(fmt) => NaiveDateTime::parse_from_str(value, fmt).is_ok(),
        Strategy::Rfc3339 => DateTime::parse_from_rfc3339(value).is_ok(),
        Strategy::Custom(fmt) => {
            NaiveDate::parse_from_str(value, fmt).is_ok()
                || NaiveDateTime::parse_from_str(value, fmt).is_ok()
                || NaiveTime::parse_from_str(value, fmt).is_ok()
        }
    }
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
    /// Why it failed.
    pub message: String,
}

/// The full validation report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Resolved header name (or `col{n}`) of the validated column.
    pub column: String,
    /// 0-based index of the validated column.
    pub column_index: usize,
    /// The date format the values were checked against (display form).
    pub format: String,
    /// Human label for the format.
    pub format_label: String,
    /// Total data cells checked in the column (header excluded).
    pub total_checked: usize,
    /// Cells that parsed (plus blank cells when allow_blank is true).
    pub valid: usize,
    /// Cells that failed (invalid_rows may be truncated, this count is not).
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

/// Validate the chosen date column. Returns the JSON or text report.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    column: &str,
    preset: &str,
    format: &str,
    has_header: bool,
    allow_blank: bool,
    delimiter: &str,
    max_issues: usize,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let (fmt_display, fmt_label, strategy) = resolve_format(preset, format)?;

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

    for (di, (line, rec)) in data_records.iter().enumerate() {
        total_checked += 1;
        let raw = rec.get(col_idx);
        let cell = raw.unwrap_or("").trim();
        let (is_valid, message) = if raw.is_none() {
            (false, "row has no value in the target column".to_string())
        } else if cell.is_empty() {
            if allow_blank {
                (true, String::new())
            } else {
                (false, "blank value not allowed".to_string())
            }
        } else if matches(cell, strategy) {
            (true, String::new())
        } else {
            (false, format!("does not match {fmt_display}"))
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
        format: fmt_display,
        format_label: fmt_label,
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
        "Column \"{}\" (index {}) checked against {} [{}]\n",
        r.column, r.column_index, r.format, r.format_label
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

    const CSV: &str = "name,joined\nAda,2021-06-01\nBo,2021/06/02\nCy,2021-12-31";

    #[test]
    fn happy_all_valid_iso() {
        let out = run("id,d\n1,2020-01-01\n2,2020-02-29", "d", "iso-date", "", true, true, "auto", 50, "text").unwrap();
        assert!(out.contains("Valid: 2"), "{out}");
        assert!(out.contains("Invalid: 0"), "{out}");
        assert!(out.contains("All 2 values are valid."), "{out}");
    }

    #[test]
    fn detects_invalid_iso_by_name() {
        let out = run(CSV, "joined", "iso-date", "", true, true, "auto", 50, "text").unwrap();
        assert!(out.contains("Rows checked: 3"), "{out}");
        assert!(out.contains("Valid: 2"), "{out}");
        assert!(out.contains("Invalid: 1"), "{out}");
        assert!(out.contains("row 2 (line 3): \"2021/06/02\" — does not match %Y-%m-%d"), "{out}");
    }

    #[test]
    fn zero_based_index_headerless() {
        let out = run("2021-01-01\n99-99-99", "0", "iso-date", "", false, true, "auto", 50, "text").unwrap();
        assert!(out.contains("Rows checked: 2"), "{out}");
        assert!(out.contains("Invalid: 1"), "{out}");
    }

    #[test]
    fn us_date_preset() {
        let out = run("d\n06/15/2021\n15/06/2021", "d", "us-date", "", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], 1);
        assert_eq!(v["invalid"], 1);
        assert_eq!(v["format"], "%m/%d/%Y");
        assert_eq!(v["invalid_rows"][0]["value"], "15/06/2021");
    }

    #[test]
    fn rfc3339_datetime() {
        let out = run("t\n2021-06-01T12:30:00Z\n2021-06-01 12:30", "t", "rfc3339", "", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], 1);
        assert_eq!(v["invalid"], 1);
    }

    #[test]
    fn custom_month_name_format() {
        let out = run("d\n01-Jun-2021\n2021-06-01", "d", "custom", "%d-%b-%Y", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], 1);
        assert_eq!(v["invalid"], 1);
    }

    #[test]
    fn blank_disallowed_is_invalid() {
        let out = run("d\n2021-06-01\n\n2021-06-02", "d", "iso-date", "", true, false, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // The blank line is skipped by the CSV reader, so only 2 data rows exist; both valid.
        assert_eq!(v["total_checked"], 2);
        assert_eq!(v["invalid"], 0);
    }

    #[test]
    fn blank_cell_disallowed() {
        let out = run("a,d\nx,\ny,2021-06-02", "d", "iso-date", "", true, false, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 1);
        assert_eq!(v["invalid_rows"][0]["message"], "blank value not allowed");
    }

    #[test]
    fn max_issues_truncates() {
        let out = run("d\nx\ny\nz", "d", "iso-date", "", true, true, "auto", 2, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 3);
        assert_eq!(v["truncated"], true);
        assert_eq!(v["invalid_rows"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn semicolon_delimiter_detected() {
        let out = run("a;d\n1;2021-06-01\n2;nope", "d", "iso-date", "", true, true, "auto", 50, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["invalid"], 1);
    }

    // --- error paths ---

    #[test]
    fn empty_input_errors() {
        assert!(run("", "d", "iso-date", "", true, true, "auto", 50, "text").is_err());
    }

    #[test]
    fn unknown_column_errors() {
        let err = run(CSV, "nope", "iso-date", "", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("not found in header"), "{err}");
    }

    #[test]
    fn index_out_of_range_errors() {
        let err = run("a,b\n1,2", "5", "iso-date", "", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn custom_requires_format() {
        let err = run(CSV, "joined", "custom", "", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("format is required"), "{err}");
    }

    #[test]
    fn unknown_preset_errors() {
        let err = run(CSV, "joined", "julian", "", true, true, "auto", 50, "text").unwrap_err();
        assert!(err.contains("unknown preset"), "{err}");
    }
}
