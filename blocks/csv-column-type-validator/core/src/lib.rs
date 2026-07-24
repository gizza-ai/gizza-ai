//! csv-column-type-validator core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! You DECLARE a type for one or more CSV columns (int / float / date / bool /
//! enum) and this checks every data cell in those columns against its declared
//! type, listing each offending cell with its row, physical line, column, value,
//! and what was expected. Report-only — the CSV is never modified. A single
//! linear parse; nothing is fetched or persisted.

use serde::Serialize;

/// Candidate delimiters tried during auto-detection, in preference order.
const CANDIDATE_DELIMS: [(char, &str); 4] =
    [(',', "comma"), ('\t', "tab"), (';', "semicolon"), ('|', "pipe")];

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// One declared column that was resolved to a real column in the data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckedColumn {
    /// Display name: the header text (header=true) or "col N" (1-based, header=false).
    pub column: String,
    /// 0-based column index in each row.
    pub col_index: usize,
    /// The declared type, human form: "int", "float", "bool", "iso date",
    /// "us date", "eu date", "date", or "enum(a|b|c)".
    pub expected: String,
}

/// One offending cell (or an empty cell when empty values are not allowed).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Offense {
    /// 1-based data-row number (1 = first row after the header).
    pub row: usize,
    /// 1-based physical line where the row starts (accounts for quoted newlines).
    pub line: usize,
    /// Column display name.
    pub column: String,
    /// 0-based column index.
    pub col_index: usize,
    /// The raw cell value (untrimmed) that failed.
    pub value: String,
    /// The declared type it failed to match (human form).
    pub expected: String,
    /// Human-readable reason.
    pub message: String,
}

/// The full validation report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True iff there are no offending cells AND every declared column was found.
    pub valid: bool,
    /// Resolved delimiter name: comma / tab / semicolon / pipe.
    pub delimiter: String,
    /// True when the delimiter was auto-detected rather than user-chosen.
    pub delimiter_detected: bool,
    /// Whether the first row was treated as a header.
    pub header: bool,
    /// The declared columns that were resolved to real columns.
    pub columns_checked: Vec<CheckedColumn>,
    /// Number of data rows examined (excludes the header and blank lines).
    pub data_rows: usize,
    /// Total cells examined (resolved columns × data rows).
    pub cells_checked: usize,
    /// Total offending cells found (NOT capped by max_issues).
    pub offending_count: usize,
    /// Declared columns that did not match any real column (typos / missing).
    pub unknown_columns: Vec<String>,
    /// The offending cells, capped at max_issues.
    pub issues: Vec<Offense>,
    /// True when more offenses were found than are listed.
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
// Declared-type parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum DeclType {
    Int,
    Float,
    Bool,
    /// The resolved date style: "iso" / "us" / "eu" / "any".
    Date(DateStyle),
    Enum(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateStyle {
    Iso,
    Us,
    Eu,
    Any,
}

impl DeclType {
    fn human(&self) -> String {
        match self {
            DeclType::Int => "int".to_string(),
            DeclType::Float => "float".to_string(),
            DeclType::Bool => "bool".to_string(),
            DeclType::Date(s) => match s {
                DateStyle::Iso => "iso date".to_string(),
                DateStyle::Us => "us date".to_string(),
                DateStyle::Eu => "eu date".to_string(),
                DateStyle::Any => "date".to_string(),
            },
            DeclType::Enum(vs) => format!("enum({})", vs.join("|")),
        }
    }
}

enum ColRef {
    Name(String),
    /// 0-based index.
    Index(usize),
}

struct Decl {
    col: ColRef,
    ty: DeclType,
}

/// Split `types` into raw declaration strings on `,` and newlines, but NOT
/// inside `enum(...)` parentheses.
fn split_decls(types: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in types.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' | '\n' if depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn parse_type_spec(spec: &str, date_style: DateStyle) -> Result<DeclType, String> {
    let low = spec.trim().to_ascii_lowercase();
    if low.starts_with("enum") {
        // enum(a|b|c) — take the values from the ORIGINAL (case-preserving) spec.
        let open = spec.find('(').ok_or_else(|| {
            format!("enum type '{spec}' must list its values, e.g. enum(active|inactive)")
        })?;
        let close = spec
            .rfind(')')
            .ok_or_else(|| format!("enum type '{spec}' is missing a closing ')'"))?;
        if close < open {
            return Err(format!("enum type '{spec}' has ')' before '('"));
        }
        let inner = &spec[open + 1..close];
        let vals: Vec<String> = inner
            .split('|')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect();
        if vals.is_empty() {
            return Err(format!(
                "enum type '{spec}' lists no values — write enum(a|b|c)"
            ));
        }
        return Ok(DeclType::Enum(vals));
    }
    Ok(match low.as_str() {
        "int" | "integer" => DeclType::Int,
        "float" | "number" | "decimal" | "double" => DeclType::Float,
        "bool" | "boolean" => DeclType::Bool,
        "date" => DeclType::Date(date_style),
        "" => return Err("a column declaration is missing its type after ':'".to_string()),
        other => {
            return Err(format!(
                "unknown type '{other}' — use int, float, bool, date, or enum(a|b|c)"
            ))
        }
    })
}

fn parse_decls(types: &str, header: bool, date_style: DateStyle) -> Result<Vec<Decl>, String> {
    let raws = split_decls(types);
    if raws.is_empty() {
        return Err(
            "declare at least one column type, e.g. age:int, joined:date, plan:enum(free|pro)"
                .to_string(),
        );
    }
    let mut decls = Vec::new();
    for raw in raws {
        // Split on the first ':' or '=' (columns/enum values never contain ':' or '=').
        let sep = raw
            .find(|ch| ch == ':' || ch == '=')
            .ok_or_else(|| format!("declaration '{raw}' must be column:type, e.g. age:int"))?;
        let (lhs, rhs) = raw.split_at(sep);
        let key = lhs.trim();
        let spec = rhs[1..].trim();
        if key.is_empty() {
            return Err(format!("declaration '{raw}' is missing a column name/index"));
        }
        let ty = parse_type_spec(spec, date_style)?;
        let col = if header {
            ColRef::Name(key.to_string())
        } else {
            let idx: usize = key.parse().map_err(|_| {
                format!("with no header, column '{key}' must be a 1-based number (e.g. 2:int)")
            })?;
            if idx == 0 {
                return Err("column indices are 1-based; use 1 for the first column".to_string());
            }
            ColRef::Index(idx - 1)
        };
        decls.push(Decl { col, ty });
    }
    Ok(decls)
}

// ---------------------------------------------------------------------------
// Type checks
// ---------------------------------------------------------------------------

fn is_int(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_float(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    // Split optional exponent.
    let (mantissa, exp) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (s, None),
    };
    if !valid_mantissa(mantissa) {
        return false;
    }
    if let Some(e) = exp {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        if e.is_empty() || !e.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    true
}

/// digits, or digits.digits, or digits. , or .digits — at least one digit overall.
fn valid_mantissa(m: &str) -> bool {
    match m.split_once('.') {
        Some((a, b)) => {
            let a_ok = a.bytes().all(|c| c.is_ascii_digit());
            let b_ok = b.bytes().all(|c| c.is_ascii_digit());
            a_ok && b_ok && !(a.is_empty() && b.is_empty())
        }
        None => !m.is_empty() && m.bytes().all(|c| c.is_ascii_digit()),
    }
}

fn is_bool(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "1" | "0" | "yes" | "no" | "t" | "f" | "y" | "n"
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn valid_ymd(y: i64, m: i64, d: i64) -> bool {
    (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}

/// Parse three numeric parts separated by `sep`, each all-digits.
fn parts(s: &str, sep: char) -> Option<[i64; 3]> {
    let mut it = s.split(sep);
    let a = it.next()?;
    let b = it.next()?;
    let c = it.next()?;
    if it.next().is_some() {
        return None;
    }
    Some([digits_only(a)?, digits_only(b)?, digits_only(c)?])
}

fn digits_only(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn is_iso_date(s: &str) -> bool {
    // YYYY-MM-DD
    match parts(s, '-') {
        Some([y, m, d]) => year_ok(s, '-', 0) && valid_ymd(y, m, d),
        None => false,
    }
}

fn is_us_date(s: &str) -> bool {
    // MM/DD/YYYY
    match parts(s, '/') {
        Some([m, d, y]) => year_ok(s, '/', 2) && valid_ymd(y, m, d),
        None => false,
    }
}

fn is_eu_date(s: &str) -> bool {
    // DD/MM/YYYY
    match parts(s, '/') {
        Some([d, m, y]) => year_ok(s, '/', 2) && valid_ymd(y, m, d),
        None => false,
    }
}

/// The year part (at position `idx` when split on `sep`) must be exactly 4 digits.
fn year_ok(s: &str, sep: char, idx: usize) -> bool {
    s.split(sep).nth(idx).map(|p| p.len() == 4).unwrap_or(false)
}

fn matches_type(value: &str, ty: &DeclType) -> bool {
    match ty {
        DeclType::Int => is_int(value),
        DeclType::Float => is_float(value),
        DeclType::Bool => is_bool(value),
        DeclType::Date(style) => match style {
            DateStyle::Iso => is_iso_date(value),
            DateStyle::Us => is_us_date(value),
            DateStyle::Eu => is_eu_date(value),
            DateStyle::Any => is_iso_date(value) || is_us_date(value) || is_eu_date(value),
        },
        DeclType::Enum(vals) => vals.iter().any(|v| v == value),
    }
}

fn parse_date_style(spec: &str) -> Result<DateStyle, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "iso" => Ok(DateStyle::Iso),
        "us" => Ok(DateStyle::Us),
        "eu" => Ok(DateStyle::Eu),
        "any" => Ok(DateStyle::Any),
        other => Err(format!(
            "unknown date_format '{other}' — use iso, us, eu, or any"
        )),
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Validate every cell in the declared columns against its declared type.
#[allow(clippy::too_many_arguments)]
pub fn validate(
    data: &str,
    types: &str,
    header: bool,
    delimiter: &str,
    date_format: &str,
    empty_ok: bool,
    max_issues: usize,
) -> Result<Report, String> {
    if data.trim().is_empty() {
        return Err("no CSV data — paste rows to validate".to_string());
    }
    let max_issues = max_issues.clamp(1, 1000);
    let (delim, delim_name, detected) = resolve_delimiter(delimiter, data)?;
    let date_style = parse_date_style(date_format)?;
    let decls = parse_decls(types, header, date_style)?;

    let rows = parse_csv(data, delim);
    if rows.is_empty() {
        return Err("no CSV rows found in the data".to_string());
    }

    // Header names (for name resolution + display).
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

    // Resolve each declared column to a 0-based index (or unknown).
    let mut columns_checked: Vec<CheckedColumn> = Vec::new();
    let mut resolved: Vec<(usize, String, DeclType)> = Vec::new(); // (col_index, display, type)
    let mut unknown_columns: Vec<String> = Vec::new();
    for decl in &decls {
        match &decl.col {
            ColRef::Name(name) => {
                let target = name.trim().to_ascii_lowercase();
                let found = header_names
                    .iter()
                    .position(|h| h.trim().to_ascii_lowercase() == target);
                match found {
                    Some(idx) => {
                        let disp = header_names[idx].trim().to_string();
                        columns_checked.push(CheckedColumn {
                            column: disp.clone(),
                            col_index: idx,
                            expected: decl.ty.human(),
                        });
                        resolved.push((idx, disp, decl.ty.clone()));
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
                        expected: decl.ty.human(),
                    });
                    resolved.push((*idx, disp, decl.ty.clone()));
                } else {
                    unknown_columns.push(format!("col {}", idx + 1));
                }
            }
        }
    }

    let data_rows = rows.len() - data_start;
    let mut issues: Vec<Offense> = Vec::new();
    let mut offending_count = 0usize;
    let mut cells_checked = 0usize;

    for (ri, row) in rows.iter().enumerate().skip(data_start) {
        let row_no = ri - data_start + 1;
        for (col_index, disp, ty) in &resolved {
            cells_checked += 1;
            let raw = row.fields.get(*col_index).map(|s| s.as_str()).unwrap_or("");
            let trimmed = raw.trim();
            let (bad, message) = if trimmed.is_empty() {
                if empty_ok {
                    (false, String::new())
                } else {
                    (true, "empty cell (required)".to_string())
                }
            } else if matches_type(trimmed, ty) {
                (false, String::new())
            } else {
                (true, format!("\"{}\" is not a valid {}", raw, ty.human()))
            };
            if bad {
                offending_count += 1;
                if issues.len() < max_issues {
                    issues.push(Offense {
                        row: row_no,
                        line: row.line,
                        column: disp.clone(),
                        col_index: *col_index,
                        value: raw.to_string(),
                        expected: ty.human(),
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
        columns_checked,
        data_rows,
        cells_checked,
        offending_count,
        unknown_columns,
        issues,
        truncated,
    })
}

/// Human-readable report for the page (`format = "text"`).
#[allow(clippy::too_many_arguments)]
pub fn summary(
    data: &str,
    types: &str,
    header: bool,
    delimiter: &str,
    date_format: &str,
    empty_ok: bool,
    max_issues: usize,
) -> Result<String, String> {
    let r = validate(data, types, header, delimiter, date_format, empty_ok, max_issues)?;

    let verdict = if r.valid {
        if r.cells_checked == 0 {
            "Valid — no data cells to check.".to_string()
        } else {
            format!(
                "Valid — all {} checked cell(s) match their declared types.",
                r.cells_checked
            )
        }
    } else if r.offending_count == 0 {
        // Only failure is unknown columns.
        "INVALID — declared column(s) not found in the data.".to_string()
    } else {
        format!("INVALID — {} offending cell(s).", r.offending_count)
    };

    let checked_list = if r.columns_checked.is_empty() {
        "no columns matched".to_string()
    } else {
        r.columns_checked
            .iter()
            .map(|c| format!("{} ({})", c.column, c.expected))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut out = format!(
        "{verdict}\nDelimiter: {}{} · {} data row(s) · {} cell(s) checked\nChecked column(s): {}\n",
        r.delimiter,
        if r.delimiter_detected {
            " (auto-detected)"
        } else {
            ""
        },
        r.data_rows,
        r.cells_checked,
        checked_list,
    );

    if !r.unknown_columns.is_empty() {
        out.push_str(&format!(
            "Declared column(s) not found: {}\n",
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
            "(+ {hidden} more offending cell(s) not shown — raise max_issues to list them)\n"
        ));
    }

    Ok(out.trim_end().to_string())
}

/// Validate CSV column types and render either a human report or JSON.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    types: &str,
    header: bool,
    delimiter: &str,
    date_format: &str,
    empty_ok: bool,
    max_issues: usize,
    format: &str,
) -> Result<String, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => summary(data, types, header, delimiter, date_format, empty_ok, max_issues),
        "json" => {
            let report = validate(data, types, header, delimiter, date_format, empty_ok, max_issues)?;
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

    fn v(data: &str, types: &str) -> Report {
        validate(data, types, true, "auto", "iso", true, 50).unwrap()
    }

    #[test]
    fn all_types_pass_on_clean_data() {
        let data = "name,age,score,active,joined,plan\n\
                    Ada,34,9.5,true,2021-06-01,pro\n\
                    Bo,7,8,no,2021-06-02,free\n";
        let r = v(data, "age:int, score:float, active:bool, joined:date, plan:enum(free|pro)");
        assert!(r.valid, "clean data should be valid: {:?}", r.issues);
        assert_eq!(r.offending_count, 0);
        assert_eq!(r.data_rows, 2);
        assert_eq!(r.cells_checked, 10); // 5 cols × 2 rows
        assert_eq!(r.columns_checked.len(), 5);
        assert_eq!(r.delimiter, "comma");
        assert!(r.delimiter_detected);
    }

    #[test]
    fn flags_offending_cells() {
        let data = "name,age,joined,plan\n\
                    Ada,34,2021-06-01,pro\n\
                    Bo,twelve,2021/06/02,gold\n";
        let r = v(data, "age:int, joined:date, plan:enum(free|pro)");
        assert!(!r.valid);
        assert_eq!(r.offending_count, 3);
        assert_eq!(r.issues.len(), 3);
        // Row 2 (Bo): age, joined, plan all wrong.
        assert!(r.issues.iter().all(|i| i.row == 2));
        let cols: Vec<&str> = r.issues.iter().map(|i| i.column.as_str()).collect();
        assert!(cols.contains(&"age"));
        assert!(cols.contains(&"joined"));
        assert!(cols.contains(&"plan"));
        assert_eq!(r.issues[0].line, 3); // physical line of row 2
    }

    #[test]
    fn empty_cells_pass_when_allowed_fail_when_required() {
        let data = "id,age\n1,\n2,30\n";
        let ok = validate(data, "age:int", true, "auto", "iso", true, 50).unwrap();
        assert!(ok.valid, "empty allowed → valid");
        let req = validate(data, "age:int", true, "auto", "iso", false, 50).unwrap();
        assert!(!req.valid);
        assert_eq!(req.offending_count, 1);
        assert_eq!(req.issues[0].message, "empty cell (required)");
    }

    #[test]
    fn unknown_declared_column_invalidates() {
        let data = "name,age\nAda,34\n";
        let r = v(data, "aeg:int"); // typo
        assert!(!r.valid);
        assert_eq!(r.offending_count, 0);
        assert_eq!(r.unknown_columns, vec!["aeg".to_string()]);
        assert!(r.columns_checked.is_empty());
    }

    #[test]
    fn headerless_uses_indices() {
        let data = "Ada,34\nBo,x\n";
        let r = validate(data, "2:int", false, "comma", "iso", true, 50).unwrap();
        assert!(!r.valid);
        assert_eq!(r.data_rows, 2);
        assert_eq!(r.offending_count, 1);
        assert_eq!(r.issues[0].column, "col 2");
        assert_eq!(r.issues[0].value, "x");
    }

    #[test]
    fn date_formats() {
        // us: MM/DD/YYYY
        let data = "d\n06/15/2021\n15/06/2021\n";
        let us = validate(data, "d:date", true, "comma", "us", true, 50).unwrap();
        assert_eq!(us.offending_count, 1); // second row invalid (month 15)
        assert_eq!(us.issues[0].row, 2);
        // any accepts iso, us, or eu
        let mixed = "d\n2021-06-15\n06/15/2021\n15/06/2021\n";
        let any = validate(mixed, "d:date", true, "comma", "any", true, 50).unwrap();
        assert!(any.valid, "any should accept all three: {:?}", any.issues);
    }

    #[test]
    fn quoted_field_with_delimiter_and_calendar_check() {
        let data = "name,when\n\"Smith, John\",2021-02-29\n";
        // 2021 is not a leap year → Feb 29 invalid.
        let r = v(data, "when:date");
        assert_eq!(r.offending_count, 1);
        assert_eq!(r.issues[0].value, "2021-02-29");
    }

    #[test]
    fn max_issues_truncates() {
        let mut data = String::from("age\n");
        for _ in 0..10 {
            data.push_str("x\n");
        }
        let r = validate(&data, "age:int", true, "comma", "iso", true, 3).unwrap();
        assert_eq!(r.offending_count, 10);
        assert_eq!(r.issues.len(), 3);
        assert!(r.truncated);
    }

    #[test]
    fn bad_type_spec_errors() {
        let err = validate("a\n1\n", "a:widget", true, "comma", "iso", true, 50).unwrap_err();
        assert!(err.contains("unknown type"), "got: {err}");
    }

    #[test]
    fn empty_types_errors() {
        let err = validate("a\n1\n", "", true, "comma", "iso", true, 50).unwrap_err();
        assert!(err.contains("declare at least one"), "got: {err}");
    }

    #[test]
    fn numeric_edge_cases() {
        assert!(is_int("-42"));
        assert!(is_int("+7"));
        assert!(!is_int("3.0"));
        assert!(!is_int("1,000"));
        assert!(!is_int(""));
        assert!(is_float("3.14"));
        assert!(is_float("-.5"));
        assert!(is_float("2."));
        assert!(is_float("1e10"));
        assert!(is_float("6.02E23"));
        assert!(is_float("42")); // int is a valid float
        assert!(!is_float("inf"));
        assert!(!is_float("nan"));
        assert!(!is_float("1.2.3"));
        assert!(!is_float("1e"));
    }

    #[test]
    fn bool_tokens() {
        for t in ["true", "FALSE", "1", "0", "Yes", "no", "T", "f", "Y", "n"] {
            assert!(is_bool(t), "{t} should be bool");
        }
        assert!(!is_bool("maybe"));
        assert!(!is_bool("2"));
    }

    #[test]
    fn summary_renders_offenses() {
        let data = "name,age\nAda,34\nBo,x\n";
        let s = summary(data, "age:int", true, "comma", "iso", true, 50).unwrap();
        assert!(s.starts_with("INVALID — 1 offending cell(s)."), "got: {s}");
        assert!(s.contains("Row 2 (line 3), column \"age\""));
        assert!(s.contains("not a valid int"));
    }

    #[test]
    fn summary_valid() {
        let s = summary("name,age\nAda,34\n", "age:int", true, "comma", "iso", true, 50).unwrap();
        assert!(s.starts_with("Valid — all 1 checked cell(s)"), "got: {s}");
    }

    #[test]
    fn enum_case_sensitive_and_trimmed_values() {
        let data = "s\nactive\nActive\n inactive \n";
        let r = v(data, "s:enum(active|inactive)");
        // "Active" (capital) is offending; " inactive " trims to inactive → ok.
        assert_eq!(r.offending_count, 1);
        assert_eq!(r.issues[0].value, "Active");
    }

    #[test]
    fn run_json_format() {
        let out = run(
            "name,age\nAda,34\nBo,x\n",
            "age:int",
            true,
            "comma",
            "iso",
            true,
            50,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], false);
        assert_eq!(v["offending_count"], 1);
        assert_eq!(v["issues"][0]["column"], "age");
    }

    #[test]
    fn run_rejects_unknown_format() {
        let err = run("a\n1\n", "a:int", true, "comma", "iso", true, 50, "xml").unwrap_err();
        assert!(err.contains("unknown format"));
    }
}
