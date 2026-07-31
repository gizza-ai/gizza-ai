//! numeric-range-check core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! You pick one or more CSV columns (by header name, 1-based index, or `all`)
//! and an expected numeric range (`min`, `max`, or both), and this flags every
//! data cell whose numeric value falls outside that range. It also reports
//! non-numeric cells (flag or ignore) and, optionally, required blank cells.
//! Report-only — the CSV is never modified. A single linear parse; nothing is
//! fetched or persisted.

use serde::Serialize;

/// Candidate delimiters tried during auto-detection, in preference order.
const CANDIDATE_DELIMS: [(char, &str); 4] = [
    (',', "comma"),
    ('\t', "tab"),
    (';', "semicolon"),
    ('|', "pipe"),
];

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// One column that was resolved to a real column in the data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckedColumn {
    /// Display name: the header text (header=true) or "col N" (1-based, header=false).
    pub column: String,
    /// 0-based column index in each row.
    pub col_index: usize,
}

/// One flagged cell.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Violation {
    /// 1-based data-row number (1 = first row after the header).
    pub row: usize,
    /// 1-based physical line where the row starts (accounts for quoted newlines).
    pub line: usize,
    /// Column display name.
    pub column: String,
    /// 0-based column index.
    pub col_index: usize,
    /// The raw cell value (untrimmed) that was flagged.
    pub value: String,
    /// Machine kind: "below", "above", "non_numeric", or "empty".
    pub kind: String,
    /// Human-readable reason.
    pub message: String,
}

/// The full range-check report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True iff there are no flagged cells AND every requested column was found.
    pub valid: bool,
    /// Resolved delimiter name: comma / tab / semicolon / pipe.
    pub delimiter: String,
    /// True when the delimiter was auto-detected rather than user-chosen.
    pub delimiter_detected: bool,
    /// Whether the first row was treated as a header.
    pub header: bool,
    /// The lower bound checked (absent = no lower bound).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The upper bound checked (absent = no upper bound).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Whether the bounds are inclusive (value == bound is in range).
    pub inclusive: bool,
    /// The columns that were resolved to real columns and checked.
    pub columns_checked: Vec<CheckedColumn>,
    /// Requested columns that did not match any real column (typos / missing).
    pub unknown_columns: Vec<String>,
    /// Number of data rows examined (excludes the header and blank lines).
    pub data_rows: usize,
    /// Total cells examined (resolved columns × data rows).
    pub cells_checked: usize,
    /// Number of cells that parsed as a number.
    pub numeric_cells: usize,
    /// Number of non-empty cells that did not parse as a number.
    pub non_numeric_cells: usize,
    /// Total flagged cells found (NOT capped by max_issues).
    pub offending_count: usize,
    /// The flagged cells, capped at max_issues.
    pub issues: Vec<Violation>,
    /// True when more flags were found than are listed.
    pub truncated: bool,
}

// ---------------------------------------------------------------------------
// CSV parsing (RFC-4180-ish, double-quote aware)
// ---------------------------------------------------------------------------

struct ParsedRow {
    /// 1-based physical line where this row starts.
    line: usize,
    fields: Vec<String>,
}

/// Parse `text` into rows using `delim`. Honors double-quoted fields (doubled
/// `""` → `"`, quoted fields may span newlines). Fully-blank physical lines are
/// skipped. A leading UTF-8 BOM is stripped.
fn parse_csv(text: &str, delim: char) -> Vec<ParsedRow> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows: Vec<ParsedRow> = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut row_started = false;
    let mut line = 1usize; // physical line of the *next* char
    let mut row_line = 1usize; // physical line where the current row started
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if !row_started {
            row_line = line;
            row_started = true;
        }
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                if c == '\n' {
                    line += 1;
                }
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            '\r' => { /* swallow; \n handles the newline */ }
            '\n' => {
                line += 1;
                fields.push(std::mem::take(&mut field));
                push_row(&mut rows, row_line, std::mem::take(&mut fields));
                row_started = false;
            }
            _ if c == delim => {
                fields.push(std::mem::take(&mut field));
            }
            _ => field.push(c),
        }
    }
    // Trailing row with no final newline.
    if row_started || !field.is_empty() || !fields.is_empty() {
        fields.push(field);
        push_row(&mut rows, row_line, fields);
    }
    rows
}

/// Push a row unless it is a single empty field (a blank physical line).
fn push_row(rows: &mut Vec<ParsedRow>, line: usize, fields: Vec<String>) {
    if fields.len() == 1 && fields[0].is_empty() {
        return;
    }
    rows.push(ParsedRow { line, fields });
}

/// Auto-detect the delimiter from the first non-blank physical line: pick the
/// candidate with the most occurrences outside quotes (comma wins ties).
fn detect_delimiter(text: &str) -> (char, &'static str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut best = (',', "comma");
    let mut best_count = -1i64;
    for (ch, name) in CANDIDATE_DELIMS {
        let mut count = 0i64;
        let mut in_q = false;
        for c in first.chars() {
            if c == '"' {
                in_q = !in_q;
            } else if c == ch && !in_q {
                count += 1;
            }
        }
        if count > best_count {
            best_count = count;
            best = (ch, name);
        }
    }
    best
}

fn resolve_delimiter(spec: &str, text: &str) -> Result<(char, &'static str, bool), String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => {
            let (c, n) = detect_delimiter(text);
            Ok((c, n, true))
        }
        "," | "comma" => Ok((',', "comma", false)),
        "\t" | "tab" => Ok(('\t', "tab", false)),
        ";" | "semicolon" => Ok((';', "semicolon", false)),
        "|" | "pipe" => Ok(('|', "pipe", false)),
        other => Err(format!(
            "unknown delimiter '{other}' — use auto, comma, tab, semicolon, or pipe"
        )),
    }
}

// ---------------------------------------------------------------------------
// Column selection + number parsing
// ---------------------------------------------------------------------------

enum ColSel {
    /// Every column.
    All,
    /// A named/indexed list.
    List(Vec<ColRef>),
}

enum ColRef {
    Name(String),
    /// 0-based index.
    Index(usize),
}

/// Split a column selector on `,` / newlines and resolve `all`/`*`, names, or
/// 1-based indices.
fn parse_columns(spec: &str, header: bool) -> Result<ColSel, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err(
            "name at least one column to check, e.g. price, or `all` for every column".to_string(),
        );
    }
    let low = trimmed.to_ascii_lowercase();
    if low == "all" || low == "*" {
        return Ok(ColSel::All);
    }
    let mut refs = Vec::new();
    for raw in spec.split([',', '\n']) {
        let key = raw.trim();
        if key.is_empty() {
            continue;
        }
        if header {
            refs.push(ColRef::Name(key.to_string()));
        } else {
            let idx: usize = key.parse().map_err(|_| {
                format!("with no header, column '{key}' must be a 1-based number (e.g. 2)")
            })?;
            if idx == 0 {
                return Err("column indices are 1-based; use 1 for the first column".to_string());
            }
            refs.push(ColRef::Index(idx - 1));
        }
    }
    if refs.is_empty() {
        return Err("name at least one column to check".to_string());
    }
    Ok(ColSel::List(refs))
}

/// Parse a decimal number, tolerating surrounding whitespace, a leading `+`,
/// underscores as digit separators, and thousands separators (`1,000`) ONLY
/// when the delimiter is not a comma. Returns None for non-numeric input,
/// NaN, or infinity.
fn parse_number(s: &str, allow_thousands: bool) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let mut cleaned = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '_' => continue,
            ',' if allow_thousands => continue,
            _ => cleaned.push(c),
        }
    }
    match cleaned.parse::<f64>() {
        Ok(v) if v.is_finite() => Some(v),
        _ => None,
    }
}

fn fmt_num(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let mut s = format!("{n}");
        if let Some(dot) = s.find('.') {
            // Trim trailing zeros for a clean display.
            let end = s.len() - s[dot..].bytes().rev().take_while(|b| *b == b'0').count();
            s.truncate(end);
            if s.ends_with('.') {
                s.pop();
            }
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Flag every cell in the selected columns whose numeric value is outside the
/// [min, max] range.
#[allow(clippy::too_many_arguments)]
pub fn check(
    data: &str,
    columns: &str,
    min: Option<f64>,
    max: Option<f64>,
    inclusive: bool,
    header: bool,
    delimiter: &str,
    non_numeric: &str,
    empty_ok: bool,
    max_issues: usize,
) -> Result<Report, String> {
    if data.trim().is_empty() {
        return Err("no CSV data — paste rows to check".to_string());
    }
    if min.is_none() && max.is_none() {
        return Err("set a min, a max, or both — there is no range to check otherwise".to_string());
    }
    if let (Some(lo), Some(hi)) = (min, max) {
        if lo > hi {
            return Err(format!(
                "min ({}) is greater than max ({}) — swap them or widen the range",
                fmt_num(lo),
                fmt_num(hi)
            ));
        }
    }
    let flag_non_numeric = match non_numeric.trim().to_ascii_lowercase().as_str() {
        "" | "flag" => true,
        "ignore" => false,
        other => {
            return Err(format!(
                "unknown non_numeric '{other}' — use flag or ignore"
            ))
        }
    };
    let max_issues = max_issues.clamp(1, 1000);
    let (delim, delim_name, detected) = resolve_delimiter(delimiter, data)?;
    let allow_thousands = delim != ',';
    let sel = parse_columns(columns, header)?;

    let rows = parse_csv(data, delim);
    if rows.is_empty() {
        return Err("no CSV rows found in the data".to_string());
    }

    let (header_names, data_start) = if header {
        (rows[0].fields.clone(), 1usize)
    } else {
        (Vec::new(), 0usize)
    };
    let width = if header {
        header_names.len()
    } else {
        rows[0].fields.len()
    };

    // Resolve requested columns to (col_index, display).
    let mut columns_checked: Vec<CheckedColumn> = Vec::new();
    let mut resolved: Vec<(usize, String)> = Vec::new();
    let mut unknown_columns: Vec<String> = Vec::new();
    match sel {
        ColSel::All => {
            for idx in 0..width {
                let disp = if header {
                    header_names[idx].trim().to_string()
                } else {
                    format!("col {}", idx + 1)
                };
                columns_checked.push(CheckedColumn {
                    column: disp.clone(),
                    col_index: idx,
                });
                resolved.push((idx, disp));
            }
        }
        ColSel::List(refs) => {
            for r in &refs {
                match r {
                    ColRef::Name(name) => {
                        let target = name.trim().to_ascii_lowercase();
                        match header_names
                            .iter()
                            .position(|h| h.trim().to_ascii_lowercase() == target)
                        {
                            Some(idx) => {
                                let disp = header_names[idx].trim().to_string();
                                columns_checked.push(CheckedColumn {
                                    column: disp.clone(),
                                    col_index: idx,
                                });
                                resolved.push((idx, disp));
                            }
                            None => unknown_columns.push(name.clone()),
                        }
                    }
                    ColRef::Index(idx) => {
                        if *idx < width {
                            let disp = format!("col {}", idx + 1);
                            columns_checked.push(CheckedColumn {
                                column: disp.clone(),
                                col_index: *idx,
                            });
                            resolved.push((*idx, disp));
                        } else {
                            unknown_columns.push(format!("col {}", idx + 1));
                        }
                    }
                }
            }
        }
    }

    let data_rows = rows.len() - data_start;
    let mut issues: Vec<Violation> = Vec::new();
    let mut offending_count = 0usize;
    let mut cells_checked = 0usize;
    let mut numeric_cells = 0usize;
    let mut non_numeric_cells = 0usize;

    for (ri, row) in rows.iter().enumerate().skip(data_start) {
        let row_no = ri - data_start + 1;
        for (col_index, disp) in &resolved {
            cells_checked += 1;
            let raw = row.fields.get(*col_index).map(|s| s.as_str()).unwrap_or("");
            let trimmed = raw.trim();
            let (bad, kind, message) = if trimmed.is_empty() {
                if empty_ok {
                    (false, "", String::new())
                } else {
                    (true, "empty", "empty cell (required)".to_string())
                }
            } else if let Some(v) = parse_number(trimmed, allow_thousands) {
                numeric_cells += 1;
                let below = min
                    .map(|lo| if inclusive { v < lo } else { v <= lo })
                    .unwrap_or(false);
                let above = max
                    .map(|hi| if inclusive { v > hi } else { v >= hi })
                    .unwrap_or(false);
                if below {
                    let lo = min.unwrap();
                    let rel = if inclusive {
                        "below min"
                    } else {
                        "not above min"
                    };
                    (
                        true,
                        "below",
                        format!("{} is {} {}", fmt_num(v), rel, fmt_num(lo)),
                    )
                } else if above {
                    let hi = max.unwrap();
                    let rel = if inclusive {
                        "above max"
                    } else {
                        "not below max"
                    };
                    (
                        true,
                        "above",
                        format!("{} is {} {}", fmt_num(v), rel, fmt_num(hi)),
                    )
                } else {
                    (false, "", String::new())
                }
            } else {
                non_numeric_cells += 1;
                if flag_non_numeric {
                    (true, "non_numeric", format!("\"{raw}\" is not a number"))
                } else {
                    (false, "", String::new())
                }
            };
            if bad {
                offending_count += 1;
                if issues.len() < max_issues {
                    issues.push(Violation {
                        row: row_no,
                        line: row.line,
                        column: disp.clone(),
                        col_index: *col_index,
                        value: raw.to_string(),
                        kind: kind.to_string(),
                        message,
                    });
                }
            }
        }
    }

    let truncated = offending_count > issues.len();
    let valid = offending_count == 0 && unknown_columns.is_empty();

    Ok(Report {
        valid,
        delimiter: delim_name.to_string(),
        delimiter_detected: detected,
        header,
        min,
        max,
        inclusive,
        columns_checked,
        unknown_columns,
        data_rows,
        cells_checked,
        numeric_cells,
        non_numeric_cells,
        offending_count,
        issues,
        truncated,
    })
}

fn range_phrase(min: Option<f64>, max: Option<f64>, inclusive: bool) -> String {
    let (lb, ub) = if inclusive {
        ("", "")
    } else {
        (" (exclusive)", " (exclusive)")
    };
    match (min, max) {
        (Some(lo), Some(hi)) => format!("{}{lb} to {}{ub}", fmt_num(lo), fmt_num(hi)),
        (Some(lo), None) => format!(
            "≥ {}{}",
            fmt_num(lo),
            if inclusive { "" } else { " (exclusive: >)" }
        ),
        (None, Some(hi)) => format!(
            "≤ {}{}",
            fmt_num(hi),
            if inclusive { "" } else { " (exclusive: <)" }
        ),
        (None, None) => "any".to_string(),
    }
}

/// Human-readable report for the page (`format = "text"`).
#[allow(clippy::too_many_arguments)]
pub fn summary(
    data: &str,
    columns: &str,
    min: Option<f64>,
    max: Option<f64>,
    inclusive: bool,
    header: bool,
    delimiter: &str,
    non_numeric: &str,
    empty_ok: bool,
    max_issues: usize,
) -> Result<String, String> {
    let r = check(
        data,
        columns,
        min,
        max,
        inclusive,
        header,
        delimiter,
        non_numeric,
        empty_ok,
        max_issues,
    )?;

    let verdict = if r.valid {
        if r.cells_checked == 0 {
            "In range — no data cells to check.".to_string()
        } else {
            format!(
                "In range — all {} checked cell(s) fall within {}.",
                r.numeric_cells,
                range_phrase(r.min, r.max, r.inclusive)
            )
        }
    } else if r.offending_count == 0 {
        "OUT OF RANGE — requested column(s) not found in the data.".to_string()
    } else {
        format!("OUT OF RANGE — {} flagged cell(s).", r.offending_count)
    };

    let checked_list = if r.columns_checked.is_empty() {
        "no columns matched".to_string()
    } else {
        r.columns_checked
            .iter()
            .map(|c| c.column.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut out = format!(
        "{verdict}\nRange: {} · Delimiter: {}{} · {} data row(s) · {} cell(s) checked\nChecked column(s): {}\n",
        range_phrase(r.min, r.max, r.inclusive),
        r.delimiter,
        if r.delimiter_detected { " (auto-detected)" } else { "" },
        r.data_rows,
        r.cells_checked,
        checked_list,
    );

    if !r.unknown_columns.is_empty() {
        out.push_str(&format!(
            "Requested column(s) not found: {}\n",
            r.unknown_columns.join(", ")
        ));
    }

    for issue in &r.issues {
        out.push_str(&format!(
            "Row {} (line {}), column \"{}\" — {}\n",
            issue.row, issue.line, issue.column, issue.message
        ));
    }

    if r.truncated {
        let hidden = r.offending_count - r.issues.len();
        out.push_str(&format!(
            "(+ {hidden} more flagged cell(s) not shown — raise max_issues to list them)\n"
        ));
    }

    Ok(out.trim_end().to_string())
}

/// Range-check CSV numeric columns and render either a human report or JSON.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    columns: &str,
    min: Option<f64>,
    max: Option<f64>,
    inclusive: bool,
    header: bool,
    delimiter: &str,
    non_numeric: &str,
    empty_ok: bool,
    max_issues: usize,
    format: &str,
) -> Result<String, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => summary(
            data,
            columns,
            min,
            max,
            inclusive,
            header,
            delimiter,
            non_numeric,
            empty_ok,
            max_issues,
        ),
        "json" => {
            let report = check(
                data,
                columns,
                min,
                max,
                inclusive,
                header,
                delimiter,
                non_numeric,
                empty_ok,
                max_issues,
            )?;
            serde_json::to_string_pretty(&report).map_err(|e| format!("failed to render JSON: {e}"))
        }
        other => Err(format!("unknown format '{other}' — use text or json")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn c(data: &str, columns: &str, min: Option<f64>, max: Option<f64>) -> Report {
        check(
            data, columns, min, max, true, true, "auto", "flag", true, 50,
        )
        .unwrap()
    }

    #[test]
    fn in_range_is_valid() {
        let data = "name,age\nAda,34\nBo,7\n";
        let r = c(data, "age", Some(0.0), Some(120.0));
        assert!(r.valid, "clean data should be in range: {:?}", r.issues);
        assert_eq!(r.offending_count, 0);
        assert_eq!(r.data_rows, 2);
        assert_eq!(r.cells_checked, 2);
        assert_eq!(r.numeric_cells, 2);
        assert_eq!(r.delimiter, "comma");
        assert!(r.delimiter_detected);
    }

    #[test]
    fn flags_below_and_above() {
        let data = "name,age\nAda,150\nBo,-3\nCy,40\n";
        let r = c(data, "age", Some(0.0), Some(120.0));
        assert!(!r.valid);
        assert_eq!(r.offending_count, 2);
        assert_eq!(r.issues[0].kind, "above");
        assert!(r.issues[0].message.contains("150 is above max 120"));
        assert_eq!(r.issues[1].kind, "below");
        assert!(r.issues[1].message.contains("-3 is below min 0"));
    }

    #[test]
    fn min_only_and_max_only() {
        let data = "v\n5\n-1\n";
        let lo = c(data, "v", Some(0.0), None);
        assert_eq!(lo.offending_count, 1);
        assert_eq!(lo.issues[0].value, "-1");
        let hi = c(data, "v", None, Some(3.0));
        assert_eq!(hi.offending_count, 1);
        assert_eq!(hi.issues[0].value, "5");
    }

    #[test]
    fn inclusive_vs_exclusive_bounds() {
        let data = "v\n0\n10\n";
        let incl = check(
            "v\n0\n10\n",
            "v",
            Some(0.0),
            Some(10.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert!(incl.valid, "0 and 10 are inside inclusive [0,10]");
        let excl = check(
            data,
            "v",
            Some(0.0),
            Some(10.0),
            false,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert_eq!(
            excl.offending_count, 2,
            "0 and 10 are outside exclusive (0,10)"
        );
        assert!(excl.issues[0].message.contains("not above min"));
        assert!(excl.issues[1].message.contains("not below max"));
    }

    #[test]
    fn non_numeric_flag_vs_ignore() {
        let data = "v\n5\nabc\n";
        let flagged = check(
            data,
            "v",
            Some(0.0),
            Some(10.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert_eq!(flagged.offending_count, 1);
        assert_eq!(flagged.issues[0].kind, "non_numeric");
        assert!(flagged.issues[0]
            .message
            .contains("\"abc\" is not a number"));
        assert_eq!(flagged.non_numeric_cells, 1);
        let ignored = check(
            data,
            "v",
            Some(0.0),
            Some(10.0),
            true,
            true,
            "comma",
            "ignore",
            true,
            50,
        )
        .unwrap();
        assert!(ignored.valid);
        assert_eq!(ignored.non_numeric_cells, 1);
    }

    #[test]
    fn all_columns_selector() {
        let data = "a,b\n5,200\n1,2\n";
        let r = check(
            data,
            "all",
            Some(0.0),
            Some(10.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert_eq!(r.columns_checked.len(), 2);
        assert_eq!(r.offending_count, 1);
        assert_eq!(r.issues[0].column, "b");
        assert_eq!(r.issues[0].value, "200");
    }

    #[test]
    fn headerless_uses_indices() {
        let data = "Ada,150\nBo,40\n";
        let r = check(
            data,
            "2",
            Some(0.0),
            Some(120.0),
            true,
            false,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert_eq!(r.data_rows, 2);
        assert_eq!(r.offending_count, 1);
        assert_eq!(r.issues[0].column, "col 2");
        assert_eq!(r.issues[0].value, "150");
    }

    #[test]
    fn empty_cells_pass_when_allowed_fail_when_required() {
        let data = "id,age\n1,\n2,30\n";
        let ok = check(
            data,
            "age",
            Some(0.0),
            Some(120.0),
            true,
            true,
            "auto",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert!(ok.valid, "empty allowed → in range");
        let req = check(
            data,
            "age",
            Some(0.0),
            Some(120.0),
            true,
            true,
            "auto",
            "flag",
            false,
            50,
        )
        .unwrap();
        assert!(!req.valid);
        assert_eq!(req.offending_count, 1);
        assert_eq!(req.issues[0].message, "empty cell (required)");
    }

    #[test]
    fn unknown_column_invalidates() {
        let data = "name,age\nAda,34\n";
        let r = c(data, "aeg", Some(0.0), Some(120.0));
        assert!(!r.valid);
        assert_eq!(r.offending_count, 0);
        assert_eq!(r.unknown_columns, vec!["aeg".to_string()]);
        assert!(r.columns_checked.is_empty());
    }

    #[test]
    fn thousands_separator_only_when_not_comma_delimited() {
        // Pipe delimiter → "1,500" is a single field that parses as 1500.
        let data = "v\n1,500\n";
        let r = check(
            data,
            "v",
            Some(0.0),
            Some(2000.0),
            true,
            true,
            "pipe",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert!(
            r.valid,
            "1,500 should parse as 1500 under pipe delimiter: {:?}",
            r.issues
        );
        assert_eq!(r.numeric_cells, 1);
    }

    #[test]
    fn decimal_and_scientific_and_underscore() {
        let data = "v\n3.14\n1e3\n1_000\n";
        let r = check(
            data,
            "v",
            Some(0.0),
            Some(2000.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert!(
            r.valid,
            "decimals, sci-notation, underscores parse: {:?}",
            r.issues
        );
        assert_eq!(r.numeric_cells, 3);
    }

    #[test]
    fn nan_and_inf_are_non_numeric() {
        let data = "v\nnan\ninf\n";
        let r = check(
            data,
            "v",
            Some(0.0),
            Some(10.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert_eq!(r.non_numeric_cells, 2);
        assert_eq!(r.offending_count, 2);
    }

    #[test]
    fn missing_bounds_errors() {
        let err = check(
            "v\n1\n", "v", None, None, true, true, "comma", "flag", true, 50,
        )
        .unwrap_err();
        assert!(err.contains("set a min"), "got: {err}");
    }

    #[test]
    fn min_greater_than_max_errors() {
        let err = check(
            "v\n1\n",
            "v",
            Some(10.0),
            Some(0.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap_err();
        assert!(err.contains("greater than max"), "got: {err}");
    }

    #[test]
    fn max_issues_truncates() {
        let mut data = String::from("v\n");
        for _ in 0..10 {
            data.push_str("999\n");
        }
        let r = check(
            &data,
            "v",
            Some(0.0),
            Some(10.0),
            true,
            true,
            "comma",
            "flag",
            true,
            3,
        )
        .unwrap();
        assert_eq!(r.offending_count, 10);
        assert_eq!(r.issues.len(), 3);
        assert!(r.truncated);
    }

    #[test]
    fn quoted_field_with_delimiter() {
        let data = "name,score\n\"Smith, John\",250\n";
        let r = c(data, "score", Some(0.0), Some(100.0));
        assert_eq!(r.offending_count, 1);
        assert_eq!(r.issues[0].value, "250");
    }

    #[test]
    fn summary_renders_flags() {
        let data = "name,age\nAda,34\nBo,200\n";
        let s = summary(
            data,
            "age",
            Some(0.0),
            Some(120.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert!(
            s.starts_with("OUT OF RANGE — 1 flagged cell(s)."),
            "got: {s}"
        );
        assert!(s.contains("Row 2 (line 3), column \"age\""));
        assert!(s.contains("200 is above max 120"));
    }

    #[test]
    fn summary_valid() {
        let s = summary(
            "name,age\nAda,34\n",
            "age",
            Some(0.0),
            Some(120.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
        )
        .unwrap();
        assert!(
            s.starts_with("In range — all 1 checked cell(s)"),
            "got: {s}"
        );
    }

    #[test]
    fn run_json_format() {
        let out = run(
            "name,age\nAda,34\nBo,200\n",
            "age",
            Some(0.0),
            Some(120.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], false);
        assert_eq!(v["offending_count"], 1);
        assert_eq!(v["issues"][0]["column"], "age");
        assert_eq!(v["issues"][0]["kind"], "above");
        assert_eq!(v["min"], 0.0);
        assert_eq!(v["max"], 120.0);
    }

    #[test]
    fn run_rejects_unknown_format() {
        let err = run(
            "v\n1\n",
            "v",
            Some(0.0),
            Some(10.0),
            true,
            true,
            "comma",
            "flag",
            true,
            50,
            "xml",
        )
        .unwrap_err();
        assert!(err.contains("unknown format"));
    }

    #[test]
    fn fmt_num_is_clean() {
        assert_eq!(fmt_num(120.0), "120");
        assert_eq!(fmt_num(-3.0), "-3");
        assert_eq!(fmt_num(3.5), "3.5");
        assert_eq!(fmt_num(0.0), "0");
    }
}
