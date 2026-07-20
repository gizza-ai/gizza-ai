//! csv-structure-validator core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Lints the RAW text of a CSV for structural faults a lenient parser would
//! swallow: ragged rows, unclosed quotes, stray quotes, blank/empty rows,
//! empty/duplicate header names, whitespace around fields, and mixed line
//! endings. Report-only — it never modifies the data. Hand-rolled RFC 4180
//! scanner (quoted fields may span lines; `""` escapes honored) because the
//! whole point is to see what forgiving CSV crates hide.

use serde::Serialize;

/// One structural finding, pointing at a physical line of the input.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Issue {
    /// 1-based physical line number (for a multi-line quoted field, the line
    /// where the row — or for unclosed_quote, the quote — starts).
    pub line: usize,
    /// "error" (breaks parsing / RFC 4180) or "warning" (suspicious but parseable).
    pub severity: String,
    /// Machine code: ragged_row, unclosed_quote, stray_quote, blank_row,
    /// empty_row, whitespace, empty_header, duplicate_header, mixed_line_endings.
    pub code: String,
    /// Human-readable description of the fault.
    pub message: String,
}

/// The full validation report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True iff no ERRORS were found (warnings alone don't fail the file).
    pub valid: bool,
    /// Resolved field separator: comma / tab / semicolon / pipe.
    pub delimiter: String,
    /// True when the delimiter was auto-detected rather than user-chosen.
    pub delimiter_detected: bool,
    /// Quote handling used: double / single / none.
    pub quote: String,
    /// Expected number of fields per row (from the header, or the first row).
    pub columns: usize,
    /// Header names (empty when header=false). Blank names stay blank here.
    pub header_columns: Vec<String>,
    /// Logical rows parsed (header + data; blank and comment lines excluded).
    pub rows: usize,
    /// Data rows (rows minus the header when header=true).
    pub data_rows: usize,
    /// Total errors found (not capped by max_issues).
    pub error_count: usize,
    /// Total warnings found (not capped by max_issues).
    pub warning_count: usize,
    /// The findings, capped at max_issues.
    pub issues: Vec<Issue>,
    /// True when more issues were found than listed.
    pub truncated: bool,
}

fn resolve_delimiter(spec: &str) -> Result<Option<char>, String> {
    Ok(match spec.trim() {
        "" | "auto" => None,
        "," | "comma" => Some(','),
        "\t" | "tab" | "\\t" => Some('\t'),
        ";" | "semicolon" => Some(';'),
        "|" | "pipe" => Some('|'),
        other => {
            let mut it = other.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Some(c),
                _ => {
                    return Err(format!(
                        "delimiter must be auto, comma, tab, semicolon, pipe, or a single character — got '{other}'"
                    ))
                }
            }
        }
    })
}

fn resolve_quote(spec: &str) -> Result<Option<char>, String> {
    Ok(match spec.trim() {
        "" | "double" | "\"" => Some('"'),
        "single" | "'" => Some('\''),
        "none" => None,
        other => return Err(format!("quote must be double, single, or none — got '{other}'")),
    })
}

fn resolve_comment(spec: &str) -> Result<Option<char>, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let mut it = s.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => Ok(Some(c)),
        _ => Err(format!("comment must be a single character (e.g. '#') — got '{s}'")),
    }
}

fn delim_name(c: char) -> String {
    match c {
        ',' => "comma".into(),
        '\t' => "tab".into(),
        ';' => "semicolon".into(),
        '|' => "pipe".into(),
        other => other.to_string(),
    }
}

/// Detect the delimiter from the first non-blank, non-comment line: count
/// candidate separators outside quotes and take the most frequent
/// (ties break comma > tab > semicolon > pipe; all-zero falls back to comma).
fn detect_delimiter(chars: &[char], quote: Option<char>, comment: Option<char>) -> char {
    let mut i = 0usize;
    let len = chars.len();
    while i < len {
        // Take one physical line.
        let start = i;
        while i < len && chars[i] != '\n' && chars[i] != '\r' {
            i += 1;
        }
        let line: &[char] = &chars[start..i];
        // Consume the line ending.
        if i < len && chars[i] == '\r' {
            i += 1;
            if i < len && chars[i] == '\n' {
                i += 1;
            }
        } else if i < len && chars[i] == '\n' {
            i += 1;
        }
        let blank = line.iter().all(|c| *c == ' ' || *c == '\t');
        let is_comment = matches!((comment, line.first()), (Some(cc), Some(f)) if *f == cc);
        if blank || is_comment {
            continue;
        }
        let mut counts = [0usize; 4]; // comma, tab, semicolon, pipe
        let mut in_quotes = false;
        for &c in line {
            if let Some(qc) = quote {
                if c == qc {
                    in_quotes = !in_quotes;
                    continue;
                }
            }
            if in_quotes {
                continue;
            }
            match c {
                ',' => counts[0] += 1,
                '\t' => counts[1] += 1,
                ';' => counts[2] += 1,
                '|' => counts[3] += 1,
                _ => {}
            }
        }
        let candidates = [',', '\t', ';', '|'];
        let mut best = 0usize;
        for (k, &n) in counts.iter().enumerate().skip(1) {
            if n > counts[best] {
                best = k;
            }
        }
        return candidates[best];
    }
    ','
}

struct FieldInfo {
    text: String,
    /// 1-based physical line where the field starts.
    line: usize,
    lead_ws: bool,
    trail_ws: bool,
    bare_quote: bool,
    junk_after_quote: bool,
}

struct RowInfo {
    /// 1-based physical line where the row starts.
    line: usize,
    fields: Vec<FieldInfo>,
    /// An unclosed quote swallowed the rest of the input from this row:
    /// (1-based field index, physical line where the quote opened).
    unclosed_quote: Option<(usize, usize)>,
}

struct Collector {
    issues: Vec<Issue>,
    error_count: usize,
    warning_count: usize,
    max_issues: usize,
}

impl Collector {
    fn push(&mut self, line: usize, severity: &str, code: &str, message: String) {
        if severity == "error" {
            self.error_count += 1;
        } else {
            self.warning_count += 1;
        }
        if self.issues.len() < self.max_issues {
            self.issues.push(Issue {
                line,
                severity: severity.into(),
                code: code.into(),
                message,
            });
        }
    }
}

/// Validate the structure of `data`.
///
/// * `has_header` — the first row names the columns (enables the empty/duplicate
///   header-name checks).
/// * `delimiter` — `auto` (detect comma/tab/semicolon/pipe) or an explicit one.
/// * `quote` — `double` (RFC 4180), `single`, or `none`.
/// * `comment` — optional single character; lines starting with it are skipped.
/// * `max_issues` — cap on the LISTED issues (1..=1000); counts stay complete.
pub fn validate(
    data: &str,
    has_header: bool,
    delimiter: &str,
    quote: &str,
    comment: &str,
    max_issues: usize,
) -> Result<Report, String> {
    if !(1..=1000).contains(&max_issues) {
        return Err(format!("max_issues must be between 1 and 1000, got {max_issues}"));
    }
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let quote_char = resolve_quote(quote)?;
    let comment_char = resolve_comment(comment)?;
    let explicit = resolve_delimiter(delimiter)?;
    if let (Some(d), Some(q)) = (explicit, quote_char) {
        if d == q {
            return Err("delimiter and quote character must differ".into());
        }
    }

    // Strip a leading BOM (documented in the page copy; not reported as an issue).
    let data = data.strip_prefix('\u{feff}').unwrap_or(data);
    let chars: Vec<char> = data.chars().collect();
    let delim = match explicit {
        Some(d) => d,
        None => detect_delimiter(&chars, quote_char, comment_char),
    };

    let mut col = Collector {
        issues: Vec::new(),
        error_count: 0,
        warning_count: 0,
        max_issues,
    };

    // Line-ending consistency, checked at every consumed newline.
    let mut first_style: Option<&'static str> = None;
    let mut mixed_reported = false;

    let len = chars.len();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut rows: Vec<RowInfo> = Vec::new();

    // Consume a line ending at `i` (if any), tracking ending style + the
    // physical line counter and reporting the first CRLF/LF/CR mix once.
    macro_rules! eat_eol {
        () => {{
            if i < len && (chars[i] == '\r' || chars[i] == '\n') {
                let style: &'static str = if chars[i] == '\r' {
                    if i + 1 < len && chars[i + 1] == '\n' {
                        i += 2;
                        "CRLF"
                    } else {
                        i += 1;
                        "CR"
                    }
                } else {
                    i += 1;
                    "LF"
                };
                match first_style {
                    None => first_style = Some(style),
                    Some(f) if f != style && !mixed_reported => {
                        mixed_reported = true;
                        col.push(
                            line,
                            "warning",
                            "mixed_line_endings",
                            format!("file mixes {f} and {style} line endings (first divergence here)"),
                        );
                    }
                    _ => {}
                }
                line += 1;
            }
        }};
    }

    while i < len {
        let row_line = line;

        // Blank line (empty, or only spaces/tabs that aren't the delimiter)?
        {
            let mut j = i;
            while j < len
                && (chars[j] == ' ' || chars[j] == '\t')
                && chars[j] != delim
            {
                j += 1;
            }
            if j >= len || chars[j] == '\n' || chars[j] == '\r' {
                i = j;
                eat_eol!();
                col.push(row_line, "warning", "blank_row", "blank line".into());
                continue;
            }
        }

        // Comment line?
        if let Some(cc) = comment_char {
            if chars[i] == cc {
                while i < len && chars[i] != '\n' && chars[i] != '\r' {
                    i += 1;
                }
                eat_eol!();
                continue;
            }
        }

        // Parse one logical row.
        let mut row = RowInfo {
            line: row_line,
            fields: Vec::new(),
            unclosed_quote: None,
        };
        'fields: loop {
            let field_idx = row.fields.len() + 1;
            let mut f = FieldInfo {
                text: String::new(),
                line,
                lead_ws: false,
                trail_ws: false,
                bare_quote: false,
                junk_after_quote: false,
            };
            // Leading spaces (tab counts only when it isn't the delimiter).
            let mut lead = String::new();
            while i < len && (chars[i] == ' ' || (chars[i] == '\t' && delim != '\t')) {
                lead.push(chars[i]);
                i += 1;
            }
            if !lead.is_empty() {
                f.lead_ws = true;
            }
            let quoted_here = matches!((quote_char, chars.get(i)), (Some(qc), Some(c)) if *c == qc);
            if quoted_here {
                let qc = quote_char.unwrap();
                let open_line = line;
                i += 1; // opening quote
                let mut closed = false;
                while i < len {
                    if chars[i] == qc {
                        if i + 1 < len && chars[i + 1] == qc {
                            f.text.push(qc);
                            i += 2;
                        } else {
                            i += 1;
                            closed = true;
                            break;
                        }
                    } else if chars[i] == '\n' || chars[i] == '\r' {
                        // Embedded newline inside a quoted field (legal).
                        eat_eol!();
                        f.text.push('\n');
                    } else {
                        f.text.push(chars[i]);
                        i += 1;
                    }
                }
                if !closed {
                    row.unclosed_quote = Some((field_idx, open_line));
                    row.fields.push(f);
                    break 'fields;
                }
                // Trailing spaces after the closing quote.
                let mut saw_trail_ws = false;
                while i < len && (chars[i] == ' ' || (chars[i] == '\t' && delim != '\t')) {
                    saw_trail_ws = true;
                    i += 1;
                }
                if saw_trail_ws {
                    f.trail_ws = true;
                }
                if i < len && chars[i] != delim && chars[i] != '\n' && chars[i] != '\r' {
                    // Junk between the closing quote and the next separator.
                    f.junk_after_quote = true;
                    while i < len && chars[i] != delim && chars[i] != '\n' && chars[i] != '\r' {
                        i += 1;
                    }
                }
            } else {
                // Unquoted field (leading spaces are part of the value).
                f.text = lead;
                while i < len && chars[i] != delim && chars[i] != '\n' && chars[i] != '\r' {
                    if let Some(qc) = quote_char {
                        if chars[i] == qc {
                            f.bare_quote = true;
                        }
                    }
                    f.text.push(chars[i]);
                    i += 1;
                }
                if f.text.ends_with(' ') || (delim != '\t' && f.text.ends_with('\t')) {
                    f.trail_ws = true;
                }
            }
            row.fields.push(f);
            if i < len && chars[i] == delim {
                i += 1;
                continue 'fields;
            }
            break 'fields;
        }
        eat_eol!();
        rows.push(row);
    }

    if rows.is_empty() {
        return Err("no rows found (only blank or comment lines)".into());
    }

    // Expected width + header names come from the first row.
    let expected = rows[0].fields.len();
    let header_columns: Vec<String> = if has_header {
        rows[0].fields.iter().map(|f| f.text.trim().to_string()).collect()
    } else {
        Vec::new()
    };

    // Per-row checks, in document order.
    for (ri, row) in rows.iter().enumerate() {
        // Quote errors first (they explain everything else on the row).
        if let Some((field_idx, open_line)) = row.unclosed_quote {
            col.push(
                open_line,
                "error",
                "unclosed_quote",
                format!("quoted field {field_idx} is opened here but never closed"),
            );
        }
        for (fi, f) in row.fields.iter().enumerate() {
            if f.bare_quote {
                col.push(
                    f.line,
                    "error",
                    "stray_quote",
                    format!(
                        "unquoted field {} contains a bare quote character — wrap the field in quotes and double any embedded quote",
                        fi + 1
                    ),
                );
            }
            if f.junk_after_quote {
                col.push(
                    f.line,
                    "error",
                    "stray_quote",
                    format!("unexpected text after the closing quote of field {}", fi + 1),
                );
            }
        }
        // Ragged check — skipped when an unclosed quote swallowed the row.
        if row.unclosed_quote.is_none() && ri > 0 && row.fields.len() != expected {
            col.push(
                row.line,
                "error",
                "ragged_row",
                format!("expected {expected} field(s), found {}", row.fields.len()),
            );
        }
        // All-empty row (right shape, no content).
        if row.fields.len() >= 2 && row.fields.iter().all(|f| f.text.trim().is_empty()) {
            col.push(
                row.line,
                "warning",
                "empty_row",
                format!("all {} field(s) are empty", row.fields.len()),
            );
        }
        // Whitespace findings, aggregated to one issue per row.
        let ws_fields: Vec<String> = row
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.lead_ws || f.trail_ws)
            .map(|(fi, _)| (fi + 1).to_string())
            .collect();
        if !ws_fields.is_empty() {
            col.push(
                row.line,
                "warning",
                "whitespace",
                format!("field(s) {} have leading or trailing space(s)", ws_fields.join(", ")),
            );
        }
    }

    // Header checks.
    if has_header {
        let empty_cols: Vec<String> = header_columns
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_empty())
            .map(|(ci, _)| (ci + 1).to_string())
            .collect();
        if !empty_cols.is_empty() {
            col.push(
                rows[0].line,
                "warning",
                "empty_header",
                format!("header column(s) {} have an empty name", empty_cols.join(", ")),
            );
        }
        for (ci, name) in header_columns.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            if let Some(first) = header_columns[..ci].iter().position(|n| n == name) {
                col.push(
                    rows[0].line,
                    "warning",
                    "duplicate_header",
                    format!("duplicate header name '{name}' (columns {} and {})", first + 1, ci + 1),
                );
            }
        }
    }

    let total_rows = rows.len();
    let data_rows = if has_header { total_rows - 1 } else { total_rows };
    let truncated = (col.error_count + col.warning_count) > col.issues.len();
    Ok(Report {
        valid: col.error_count == 0,
        delimiter: delim_name(delim),
        delimiter_detected: explicit.is_none(),
        quote: match quote_char {
            Some('"') => "double".into(),
            Some('\'') => "single".into(),
            Some(c) => c.to_string(),
            None => "none".into(),
        },
        columns: expected,
        header_columns,
        rows: total_rows,
        data_rows,
        error_count: col.error_count,
        warning_count: col.warning_count,
        issues: col.issues,
        truncated,
    })
}

/// Plain-text report (used by the page) — verdict, dialect line, one line per issue.
pub fn summary(
    data: &str,
    has_header: bool,
    delimiter: &str,
    quote: &str,
    comment: &str,
    max_issues: usize,
) -> Result<String, String> {
    let r = validate(data, has_header, delimiter, quote, comment, max_issues)?;
    let verdict = if r.valid {
        if r.warning_count == 0 {
            "Valid CSV — no structural problems found.".to_string()
        } else {
            format!("Valid CSV — no errors, {} warning(s).", r.warning_count)
        }
    } else {
        format!("INVALID CSV — {} error(s), {} warning(s).", r.error_count, r.warning_count)
    };
    let mut out = format!(
        "{verdict}\nDelimiter: {}{} · Quote: {} · Expected {} field(s) per row · {} data row(s)\n",
        r.delimiter,
        if r.delimiter_detected { " (auto-detected)" } else { "" },
        r.quote,
        r.columns,
        r.data_rows,
    );
    for issue in &r.issues {
        out.push_str(&format!(
            "Line {} [{}] {} — {}\n",
            issue.line, issue.severity, issue.code, issue.message
        ));
    }
    if r.truncated {
        let hidden = r.error_count + r.warning_count - r.issues.len();
        out.push_str(&format!(
            "(+ {hidden} more issue(s) not shown — raise max_issues to list them)\n"
        ));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(r: &Report) -> Vec<(&str, usize)> {
        r.issues.iter().map(|i| (i.code.as_str(), i.line)).collect()
    }

    #[test]
    fn clean_csv_is_valid() {
        let r = validate("a,b,c\n1,2,3\n4,5,6\n", true, "auto", "double", "", 50).unwrap();
        assert!(r.valid);
        assert_eq!(r.error_count, 0);
        assert_eq!(r.warning_count, 0);
        assert!(r.issues.is_empty());
        assert_eq!(r.columns, 3);
        assert_eq!(r.rows, 3);
        assert_eq!(r.data_rows, 2);
        assert_eq!(r.delimiter, "comma");
        assert!(r.delimiter_detected);
        assert_eq!(r.header_columns, vec!["a", "b", "c"]);
        assert!(!r.truncated);
    }

    #[test]
    fn ragged_rows_flagged_with_lines() {
        let r = validate("a,b,c\n1,2\n3,4,5,6\n", true, "comma", "double", "", 50).unwrap();
        assert!(!r.valid);
        assert_eq!(r.error_count, 2);
        assert_eq!(codes(&r), vec![("ragged_row", 2), ("ragged_row", 3)]);
        assert!(r.issues[0].message.contains("expected 3 field(s), found 2"));
        assert!(r.issues[1].message.contains("found 4"));
        assert!(!r.delimiter_detected);
    }

    #[test]
    fn unclosed_quote_reported_at_opening_line_and_suppresses_ragged() {
        let d = "a,b\n1,\"open\nmore,stuff\n";
        let r = validate(d, true, "comma", "double", "", 50).unwrap();
        assert!(!r.valid);
        assert_eq!(r.error_count, 1);
        assert_eq!(codes(&r), vec![("unclosed_quote", 2)]);
        assert!(r.issues[0].message.contains("field 2"));
    }

    #[test]
    fn stray_quote_bare_and_after_closing() {
        let d = "a,b\nx\"y,2\n\"ok\"junk,4\n";
        let r = validate(d, true, "comma", "double", "", 50).unwrap();
        assert_eq!(r.error_count, 2);
        assert_eq!(codes(&r), vec![("stray_quote", 2), ("stray_quote", 3)]);
        assert!(r.issues[0].message.contains("bare quote"));
        assert!(r.issues[1].message.contains("after the closing quote"));
    }

    #[test]
    fn quoted_delimiters_and_multiline_fields_are_fine() {
        let d = "name,note\n\"Smith, John\",\"line one\nline two\"\nBo,plain\n";
        let r = validate(d, true, "comma", "double", "", 50).unwrap();
        assert!(r.valid, "issues: {:?}", r.issues);
        assert_eq!(r.rows, 3);
        assert_eq!(r.data_rows, 2);
    }

    #[test]
    fn escaped_quotes_are_fine() {
        let r =
            validate("a,b\n\"he said \"\"hi\"\"\",2\n", true, "comma", "double", "", 50).unwrap();
        assert!(r.valid, "issues: {:?}", r.issues);
    }

    #[test]
    fn blank_and_empty_rows_are_warnings() {
        let d = "a,b\n1,2\n\n,\n3,4\n";
        let r = validate(d, true, "comma", "double", "", 50).unwrap();
        assert!(r.valid); // warnings only
        assert_eq!(r.warning_count, 2);
        assert_eq!(codes(&r), vec![("blank_row", 3), ("empty_row", 4)]);
        assert_eq!(r.data_rows, 3); // the "," row still parses as a data row
    }

    #[test]
    fn trailing_final_newline_is_not_a_blank_row() {
        let r = validate("a,b\n1,2\n", true, "comma", "double", "", 50).unwrap();
        assert_eq!(r.warning_count, 0, "issues: {:?}", r.issues);
    }

    #[test]
    fn header_checks() {
        let d = "id,,id,name\n1,2,3,4\n";
        let r = validate(d, true, "comma", "double", "", 50).unwrap();
        assert!(r.valid);
        assert_eq!(codes(&r), vec![("empty_header", 1), ("duplicate_header", 1)]);
        assert!(r.issues[0].message.contains("column(s) 2"));
        assert!(r.issues[1].message.contains("'id' (columns 1 and 3)"));
        // header=false: same data, no header findings.
        let r2 = validate(d, false, "comma", "double", "", 50).unwrap();
        assert_eq!(r2.issues.len(), 0);
        assert!(r2.header_columns.is_empty());
        assert_eq!(r2.data_rows, 2);
    }

    #[test]
    fn whitespace_aggregated_per_row() {
        let d = "a,b,c\n 1,2 , 3\nx, \"q\" ,y\n";
        let r = validate(d, true, "comma", "double", "", 50).unwrap();
        assert!(r.valid);
        let ws: Vec<&Issue> = r.issues.iter().filter(|i| i.code == "whitespace").collect();
        assert_eq!(ws.len(), 2);
        assert!(ws[0].message.contains("field(s) 1, 2, 3"));
        assert!(ws[1].message.contains("field(s) 2"));
    }

    #[test]
    fn mixed_line_endings_reported_once() {
        let d = "a,b\r\n1,2\n3,4\r\n";
        let r = validate(d, true, "comma", "double", "", 50).unwrap();
        let mixed: Vec<&Issue> =
            r.issues.iter().filter(|i| i.code == "mixed_line_endings").collect();
        assert_eq!(mixed.len(), 1);
        assert!(mixed[0].message.contains("CRLF and LF"));
    }

    #[test]
    fn delimiter_auto_detects_semicolon_and_tab() {
        let r = validate("a;b;c\n1;2;3\n", true, "auto", "double", "", 50).unwrap();
        assert_eq!(r.delimiter, "semicolon");
        assert!(r.delimiter_detected);
        assert!(r.valid);
        let r = validate("a\tb\n1\t2\n", true, "auto", "double", "", 50).unwrap();
        assert_eq!(r.delimiter, "tab");
        assert!(r.valid, "issues: {:?}", r.issues);
    }

    #[test]
    fn single_quote_and_none_modes() {
        let d = "a,b\n'x,y',2\n";
        let r = validate(d, true, "comma", "single", "", 50).unwrap();
        assert!(r.valid, "issues: {:?}", r.issues);
        // Same data under quote=none: the ' is plain text, the comma splits → ragged.
        let r2 = validate(d, true, "comma", "none", "", 50).unwrap();
        assert!(!r2.valid);
        assert_eq!(r2.issues[0].code, "ragged_row");
    }

    #[test]
    fn comment_lines_skipped() {
        let d = "# a comment\na,b\n# another\n1,2\n";
        let r = validate(d, true, "comma", "double", "#", 50).unwrap();
        assert!(r.valid, "issues: {:?}", r.issues);
        assert_eq!(r.rows, 2);
        // Without the comment char those lines are single-field ragged rows.
        let r2 = validate(d, true, "comma", "double", "", 50).unwrap();
        assert!(!r2.valid);
    }

    #[test]
    fn max_issues_caps_list_but_not_counts() {
        let d = "a,b\n1\n2\n3\n4\n";
        let r = validate(d, true, "comma", "double", "", 2).unwrap();
        assert_eq!(r.error_count, 4);
        assert_eq!(r.issues.len(), 2);
        assert!(r.truncated);
    }

    #[test]
    fn max_issues_bounds_enforced() {
        assert!(validate("a,b\n1,2", true, "comma", "double", "", 0).is_err());
        assert!(validate("a,b\n1,2", true, "comma", "double", "", 1001).is_err());
        assert!(validate("a,b\n1,2", true, "comma", "double", "", 1000).is_ok());
    }

    #[test]
    fn bom_is_stripped() {
        let r = validate("\u{feff}a,b\n1,2\n", true, "comma", "double", "", 50).unwrap();
        assert!(r.valid);
        assert_eq!(r.header_columns, vec!["a", "b"]);
    }

    #[test]
    fn empty_input_and_bad_args_error() {
        assert!(validate("", true, "comma", "double", "", 50).is_err());
        assert!(validate("   \n  ", true, "comma", "double", "", 50).is_err());
        assert!(validate("a,b", true, "nope!", "double", "", 50).is_err());
        assert!(validate("a,b", true, "comma", "fancy", "", 50).is_err());
        assert!(validate("a,b", true, "comma", "double", "##", 50).is_err());
        // Explicit delimiter equal to the quote char is rejected.
        assert!(validate("a,b", true, "\"", "double", "", 50).is_err());
    }

    #[test]
    fn summary_reports_verdict_and_lines() {
        let s = summary("a,b,c\n1,2\n", true, "auto", "double", "", 50).unwrap();
        assert!(s.starts_with("INVALID CSV — 1 error(s), 0 warning(s)."));
        assert!(s.contains(
            "Delimiter: comma (auto-detected) · Quote: double · Expected 3 field(s) per row · 1 data row(s)"
        ));
        assert!(s.contains("Line 2 [error] ragged_row — expected 3 field(s), found 2"));
        let ok = summary("a,b\n1,2\n", true, "comma", "double", "", 50).unwrap();
        assert_eq!(
            ok,
            "Valid CSV — no structural problems found.\nDelimiter: comma · Quote: double · Expected 2 field(s) per row · 1 data row(s)"
        );
    }

    #[test]
    fn summary_notes_truncation() {
        let s = summary("a,b\n1\n2\n3\n", true, "comma", "double", "", 1).unwrap();
        assert!(s.contains("(+ 2 more issue(s) not shown — raise max_issues to list them)"));
    }
}
