//! csv-type-inferrer core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Sniffs a CSV's delimiter/quote dialect, detects a header row, infers a type
//! for every column (int / float / bool / date / datetime / string / empty), and
//! emits a JSON schema report and/or typed JSON records. All parsing is done in
//! a single linear pass; nothing is fetched or persisted.

use serde_json::{json, Map, Value};

/// Candidate delimiters tried during auto-detection, in preference order.
const CANDIDATE_DELIMS: [(char, &str); 4] =
    [(',', "comma"), (';', "semicolon"), ('\t', "tab"), ('|', "pipe")];

/// The inferred type of a column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColType {
    /// Column has no non-null values.
    Empty,
    Bool,
    Int,
    Float,
    /// Every value matched the named date format (e.g. `YYYY-MM-DD`).
    Date(&'static str),
    /// Every value matched the named datetime format.
    Datetime(&'static str),
    Str,
}

impl ColType {
    fn name(&self) -> &'static str {
        match self {
            ColType::Empty => "empty",
            ColType::Bool => "bool",
            ColType::Int => "int",
            ColType::Float => "float",
            ColType::Date(_) => "date",
            ColType::Datetime(_) => "datetime",
            ColType::Str => "string",
        }
    }
    fn format(&self) -> Option<&'static str> {
        match self {
            ColType::Date(f) | ColType::Datetime(f) => Some(f),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// CSV parsing (RFC-4180-ish, dialect + quote aware)
// ---------------------------------------------------------------------------

/// Parse `text` into rows of fields using `delim` and `quote`. Handles doubled
/// quotes (`""` → `"`) and quoted fields that span newlines. Fully-empty rows
/// (blank lines) are skipped. Surrounding whitespace of each field is trimmed.
pub fn parse_csv(text: &str, delim: char, quote: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut field_started = false; // any char (incl. an opening quote) seen for this field
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;

    let push_field = |field: &mut String, row: &mut Vec<String>| {
        row.push(std::mem::take(field).trim().to_string());
    };

    while i < n {
        let c = chars[i];
        if in_quotes {
            if c == quote {
                if i + 1 < n && chars[i + 1] == quote {
                    field.push(quote);
                    i += 2;
                    continue;
                }
                in_quotes = false;
                i += 1;
                continue;
            }
            field.push(c);
            i += 1;
            continue;
        }
        if c == quote && field.is_empty() {
            in_quotes = true;
            field_started = true;
            i += 1;
        } else if c == delim {
            push_field(&mut field, &mut row);
            field_started = true; // a delimiter guarantees a following (possibly empty) field
            i += 1;
        } else if c == '\n' || c == '\r' {
            if field_started || !field.is_empty() || !row.is_empty() {
                push_field(&mut field, &mut row);
                rows.push(std::mem::take(&mut row));
            }
            field_started = false;
            if c == '\r' && i + 1 < n && chars[i + 1] == '\n' {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            field.push(c);
            field_started = true;
            i += 1;
        }
    }
    if field_started || !field.is_empty() || !row.is_empty() {
        push_field(&mut field, &mut row);
        rows.push(row);
    }

    // Drop blank lines (a row whose every field is empty).
    rows.retain(|r| !r.iter().all(|f| f.is_empty()));
    rows
}

/// Score how consistently `text` splits into columns under `delim`. Returns
/// `(modal_column_count, fraction_of_rows_at_modal)`.
fn delim_score(text: &str, delim: char) -> (usize, f64) {
    let rows = parse_csv(text, delim, '"');
    if rows.is_empty() {
        return (0, 0.0);
    }
    let mut freq: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for r in &rows {
        *freq.entry(r.len()).or_insert(0) += 1;
    }
    let (&modal, &hits) = freq.iter().max_by_key(|(_, &c)| c).unwrap();
    (modal, hits as f64 / rows.len() as f64)
}

/// Pick the delimiter that yields the most consistent, widest table.
pub fn sniff_delimiter(text: &str) -> char {
    let mut best: Option<(char, usize, f64)> = None;
    for (d, _) in CANDIDATE_DELIMS {
        let (modal, consistency) = delim_score(text, d);
        if modal < 2 {
            continue; // a single column means this delimiter never split anything
        }
        let better = match best {
            None => true,
            Some((_, bm, bc)) => (consistency, modal) > (bc, bm),
        };
        if better {
            best = Some((d, modal, consistency));
        }
    }
    best.map(|(d, _, _)| d).unwrap_or(',')
}

/// Detect the field-quoting character (`"` or `'`) by counting quotes that open
/// a field. Defaults to `"`.
pub fn sniff_quote(text: &str, delim: char) -> char {
    let mut dq = 0i64;
    let mut sq = 0i64;
    let mut at_field_start = true;
    for c in text.chars() {
        if at_field_start {
            if c == '"' {
                dq += 1;
            } else if c == '\'' {
                sq += 1;
            }
        }
        at_field_start = c == delim || c == '\n' || c == '\r';
    }
    if sq > dq {
        '\''
    } else {
        '"'
    }
}

// ---------------------------------------------------------------------------
// Scalar recognizers
// ---------------------------------------------------------------------------

fn is_true(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "true" | "yes")
}
fn is_bool(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no"
    )
}

/// Parse a base-10 integer, but reject zero-padded codes (`007`, `01`) so ZIP
/// codes / IDs stay strings rather than losing their leading zero.
fn parse_int(s: &str) -> Option<i64> {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if body.len() > 1 && body.starts_with('0') {
        return None; // leading-zero code, not a numeric int
    }
    s.parse::<i64>().ok()
}

/// Parse a finite float. Rejects textual `nan`/`inf` (which are null markers or
/// labels, not numbers).
fn parse_float(s: &str) -> Option<f64> {
    match s.to_ascii_lowercase().as_str() {
        "nan" | "inf" | "+inf" | "-inf" | "infinity" | "+infinity" | "-infinity" => return None,
        _ => {}
    }
    // Reject zero-padded all-digit codes (`007`, `01234`) so ZIP codes / IDs stay
    // strings, matching parse_int — a genuine float always carries a `.`/`e`/`E`.
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if body.len() > 1 && body.starts_with('0') && body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match s.parse::<f64>() {
        Ok(v) if v.is_finite() => Some(v),
        _ => None,
    }
}

// ---- date / datetime recognizers ----

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
    (1..=9999).contains(&y) && (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}
fn split3(s: &str, sep: char) -> Option<(&str, &str, &str)> {
    let mut it = s.split(sep);
    let a = it.next()?;
    let b = it.next()?;
    let c = it.next()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}
/// Exactly `len` ASCII digits → the parsed number.
fn digits(s: &str, len: usize) -> Option<i64> {
    if s.len() == len && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

/// Validate the three fields of a date given the format's component order.
fn date_from(a: &str, b: &str, c: &str, order: &str) -> bool {
    match order {
        "ymd" => match (digits(a, 4), digits(b, 2), digits(c, 2)) {
            (Some(y), Some(m), Some(d)) => valid_ymd(y, m, d),
            _ => false,
        },
        "mdy" => match (digits(a, 2), digits(b, 2), digits(c, 4)) {
            (Some(m), Some(d), Some(y)) => valid_ymd(y, m, d),
            _ => false,
        },
        "dmy" => match (digits(a, 2), digits(b, 2), digits(c, 4)) {
            (Some(d), Some(m), Some(y)) => valid_ymd(y, m, d),
            _ => false,
        },
        _ => false,
    }
}

/// Ordered list of supported date formats: `(friendly_name, separator, order)`.
const DATE_FORMATS: [(&str, char, &str); 6] = [
    ("YYYY-MM-DD", '-', "ymd"),
    ("YYYY/MM/DD", '/', "ymd"),
    ("MM/DD/YYYY", '/', "mdy"),
    ("DD/MM/YYYY", '/', "dmy"),
    ("MM-DD-YYYY", '-', "mdy"),
    ("DD-MM-YYYY", '-', "dmy"),
];

fn match_date(s: &str, sep: char, order: &str) -> bool {
    match split3(s, sep) {
        Some((a, b, c)) => date_from(a, b, c, order),
        None => false,
    }
}

fn valid_time(tp: &str) -> bool {
    let (main, frac) = match tp.split_once('.') {
        Some((m, f)) => (m, Some(f)),
        None => (tp, None),
    };
    if let Some(f) = frac {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    if let Some((h, mi, se)) = split3(main, ':') {
        if let (Some(h), Some(mi), Some(se)) = (digits(h, 2), digits(mi, 2), digits(se, 2)) {
            return (0..=23).contains(&h) && (0..=59).contains(&mi) && (0..=60).contains(&se);
        }
    }
    false
}

/// Ordered list of supported datetime formats: `(friendly_name, kind)`.
const DATETIME_FORMATS: [(&str, &str); 3] = [
    ("YYYY-MM-DDTHH:MM:SSZ", "tz"),
    ("YYYY-MM-DDTHH:MM:SS", "t"),
    ("YYYY-MM-DD HH:MM:SS", "space"),
];

fn match_datetime(s: &str, kind: &str) -> bool {
    match kind {
        "tz" => match s.strip_suffix('Z') {
            Some(core) => match core.find('T') {
                Some(i) => match_date(&core[..i], '-', "ymd") && valid_time(&core[i + 1..]),
                None => false,
            },
            None => false,
        },
        "t" => {
            if s.ends_with('Z') {
                return false;
            }
            match s.find('T') {
                Some(i) => match_date(&s[..i], '-', "ymd") && valid_time(&s[i + 1..]),
                None => false,
            }
        }
        "space" => {
            if s.contains('T') {
                return false;
            }
            match s.find(' ') {
                Some(i) => match_date(&s[..i], '-', "ymd") && valid_time(&s[i + 1..]),
                None => false,
            }
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Column inference
// ---------------------------------------------------------------------------

/// Infer the type of a column from its non-null cell values.
pub fn infer_col(cells: &[&str], date_detection: bool) -> ColType {
    if cells.is_empty() {
        return ColType::Empty;
    }
    if cells.iter().all(|c| is_bool(c)) {
        return ColType::Bool;
    }
    if cells.iter().all(|c| parse_int(c).is_some()) {
        return ColType::Int;
    }
    if cells
        .iter()
        .all(|c| parse_int(c).is_some() || parse_float(c).is_some())
    {
        return ColType::Float;
    }
    if date_detection {
        for (name, sep, order) in DATE_FORMATS {
            if cells.iter().all(|c| match_date(c, sep, order)) {
                return ColType::Date(name);
            }
        }
        for (name, kind) in DATETIME_FORMATS {
            if cells.iter().all(|c| match_datetime(c, kind)) {
                return ColType::Datetime(name);
            }
        }
    }
    ColType::Str
}

/// Coerce a single (already-trimmed) cell to its typed JSON value.
fn typed_value(cell: Option<&str>, ct: &ColType) -> Value {
    let c = match cell {
        None => return Value::Null,
        Some(c) => c,
    };
    match ct {
        ColType::Int => parse_int(c)
            .map(|i| json!(i))
            .unwrap_or_else(|| Value::String(c.to_string())),
        // Render integral cells in a float column as integers (`7`, not `7.0`);
        // fall back to the float value, then to the raw string.
        ColType::Float => parse_int(c)
            .map(|i| json!(i))
            .or_else(|| parse_float(c).map(|f| json!(f)))
            .unwrap_or_else(|| Value::String(c.to_string())),
        ColType::Bool => Value::Bool(is_true(c)),
        _ => Value::String(c.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Options + entry point
// ---------------------------------------------------------------------------

fn resolve_delim(delimiter: &str, text: &str) -> Result<char, String> {
    match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(sniff_delimiter(text)),
        "comma" | "," => Ok(','),
        "tab" | "\\t" | "\t" => Ok('\t'),
        "semicolon" | ";" => Ok(';'),
        "pipe" | "|" => Ok('|'),
        other => Err(format!(
            "unknown delimiter '{other}' (use auto, comma, tab, semicolon, or pipe)"
        )),
    }
}

/// `true` if the trimmed cell should count as null/missing.
fn is_null(cell: &str, null_set: &[String]) -> bool {
    cell.is_empty() || null_set.iter().any(|t| t == cell)
}

/// Header detection (auto): vote header when a column's data rows infer to a
/// definite non-string type but the first row's cell for that column does not
/// match it; vote against when the first row already matches. Ties / all-string
/// tables default to "header present".
fn detect_header(rows: &[Vec<String>], null_set: &[String], date_detection: bool) -> bool {
    if rows.len() < 2 {
        return rows.len() == 1; // a lone line is treated as a header with no records
    }
    let ncol = rows[0].len();
    let mut votes = 0i64;
    let mut had_typed = false;
    for c in 0..ncol {
        let data: Vec<&str> = rows[1..]
            .iter()
            .filter_map(|r| r.get(c).map(|s| s.as_str()))
            .filter(|s| !is_null(s, null_set))
            .collect();
        let ct = infer_col(&data, date_detection);
        if matches!(
            ct,
            ColType::Int | ColType::Float | ColType::Bool | ColType::Date(_) | ColType::Datetime(_)
        ) {
            had_typed = true;
            let head = rows[0].get(c).map(|s| s.as_str()).unwrap_or("");
            let head_matches =
                !is_null(head, null_set) && infer_col(&[head], date_detection).name() == ct.name();
            if head_matches {
                votes -= 1;
            } else {
                votes += 1;
            }
        }
    }
    if !had_typed {
        return true; // all-string table: can't tell, assume the first row names columns
    }
    votes > 0
}

/// Build unique, non-empty column names from a header row.
fn header_names(header: &[String], ncol: usize) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(ncol);
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for i in 0..ncol {
        let raw = header.get(i).map(|s| s.trim()).unwrap_or("");
        let base = if raw.is_empty() {
            format!("column_{}", i + 1)
        } else {
            raw.to_string()
        };
        let name = match seen.get_mut(&base) {
            Some(n) => {
                *n += 1;
                format!("{base}_{n}")
            }
            None => {
                seen.insert(base.clone(), 1);
                base
            }
        };
        names.push(name);
    }
    names
}

/// Infer a CSV's dialect + column types and emit a JSON report.
///
/// * `data` — the raw CSV text.
/// * `delimiter` — `auto` (sniff) or `comma`/`tab`/`semicolon`/`pipe`.
/// * `headers` — `auto` (detect), `present`, or `absent`.
/// * `null_tokens` — comma-separated tokens (plus the empty string) treated as
///   missing values, e.g. `NA,N/A,NULL`.
/// * `date_detection` — when true, recognize date/datetime columns.
/// * `output` — `both` (schema + records), `schema`, or `records`.
pub fn infer(
    data: &str,
    delimiter: &str,
    headers: &str,
    null_tokens: &str,
    date_detection: bool,
    output: &str,
) -> Result<String, String> {
    let output = match output.trim().to_ascii_lowercase().as_str() {
        "" | "both" => "both",
        "schema" => "schema",
        "records" => "records",
        other => {
            return Err(format!(
                "unknown output '{other}' (use both, schema, or records)"
            ))
        }
    };
    let header_mode = match headers.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => "auto",
        "present" | "true" | "yes" => "present",
        "absent" | "false" | "no" | "none" => "absent",
        other => {
            return Err(format!(
                "unknown headers '{other}' (use auto, present, or absent)"
            ))
        }
    };

    if data.trim().is_empty() {
        return Err("no CSV data provided".into());
    }
    let delim = resolve_delim(delimiter, data)?;
    let quote = sniff_quote(data, delim);

    let rows = parse_csv(data, delim, quote);
    if rows.is_empty() {
        return Err("no rows found in the input".into());
    }

    let null_set: Vec<String> = null_tokens
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    let has_header = match header_mode {
        "present" => true,
        "absent" => false,
        _ => detect_header(&rows, &null_set, date_detection),
    };

    let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let names = if has_header {
        header_names(&rows[0], ncol)
    } else {
        (1..=ncol).map(|i| format!("column_{i}")).collect()
    };
    let data_rows: &[Vec<String>] = if has_header { &rows[1..] } else { &rows[..] };

    // Per-column inference + counts.
    let mut columns: Vec<Value> = Vec::with_capacity(ncol);
    let mut coltypes: Vec<ColType> = Vec::with_capacity(ncol);
    for c in 0..ncol {
        let raw: Vec<&str> = data_rows
            .iter()
            .map(|r| r.get(c).map(|s| s.as_str()).unwrap_or(""))
            .collect();
        let non_null: Vec<&str> = raw
            .iter()
            .copied()
            .filter(|s| !is_null(s, &null_set))
            .collect();
        let missing = raw.len() - non_null.len();
        let unique: std::collections::HashSet<&str> = non_null.iter().copied().collect();
        let ct = infer_col(&non_null, date_detection);

        let mut col = Map::new();
        col.insert("name".into(), json!(names[c]));
        col.insert("type".into(), json!(ct.name()));
        if let Some(f) = ct.format() {
            col.insert("format".into(), json!(f));
        }
        col.insert("nullable".into(), json!(missing > 0));
        col.insert("non_empty".into(), json!(non_null.len()));
        col.insert("missing".into(), json!(missing));
        col.insert("unique".into(), json!(unique.len()));
        columns.push(Value::Object(col));
        coltypes.push(ct);
    }

    if output == "records" {
        let recs = build_records(data_rows, &names, &coltypes, &null_set, ncol);
        return serde_json::to_string_pretty(&Value::Array(recs)).map_err(|e| e.to_string());
    }

    let mut root = Map::new();
    let mut dialect = Map::new();
    let delim_name = CANDIDATE_DELIMS
        .iter()
        .find(|(d, _)| *d == delim)
        .map(|(_, n)| *n)
        .unwrap_or("custom");
    dialect.insert("delimiter".into(), json!(delim.to_string()));
    dialect.insert("delimiter_name".into(), json!(delim_name));
    dialect.insert("quote_char".into(), json!(quote.to_string()));
    dialect.insert("has_header".into(), json!(has_header));
    root.insert("dialect".into(), Value::Object(dialect));
    root.insert("row_count".into(), json!(data_rows.len()));
    root.insert("column_count".into(), json!(ncol));
    root.insert("columns".into(), Value::Array(columns));
    if output == "both" {
        let recs = build_records(data_rows, &names, &coltypes, &null_set, ncol);
        root.insert("records".into(), Value::Array(recs));
    }
    serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())
}

fn build_records(
    data_rows: &[Vec<String>],
    names: &[String],
    coltypes: &[ColType],
    null_set: &[String],
    ncol: usize,
) -> Vec<Value> {
    data_rows
        .iter()
        .map(|r| {
            let mut obj = Map::new();
            for c in 0..ncol {
                let cell = r.get(c).map(|s| s.as_str());
                let val = match cell {
                    Some(s) if !is_null(s, null_set) => typed_value(Some(s), &coltypes[c]),
                    _ => Value::Null,
                };
                obj.insert(names[c].clone(), val);
            }
            Value::Object(obj)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(v: &Value, ptr: &str) -> Value {
        v.pointer(ptr).cloned().unwrap_or(Value::Null)
    }

    #[test]
    fn happy_mixed_types_with_header() {
        let csv =
            "id,name,active,joined,score\n1,Alice,true,2021-05-01,9.5\n2,Bob,false,2022-11-30,7\n";
        let out = infer(csv, "auto", "auto", "NA,NULL", true, "both").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(get(&v, "/dialect/delimiter_name"), json!("comma"));
        assert_eq!(get(&v, "/dialect/has_header"), json!(true));
        assert_eq!(get(&v, "/row_count"), json!(2));
        assert_eq!(get(&v, "/column_count"), json!(5));
        assert_eq!(get(&v, "/columns/0/type"), json!("int"));
        assert_eq!(get(&v, "/columns/1/type"), json!("string"));
        assert_eq!(get(&v, "/columns/2/type"), json!("bool"));
        assert_eq!(get(&v, "/columns/3/type"), json!("date"));
        assert_eq!(get(&v, "/columns/3/format"), json!("YYYY-MM-DD"));
        assert_eq!(get(&v, "/columns/4/type"), json!("float"));
        // typed records
        assert_eq!(get(&v, "/records/0/id"), json!(1));
        assert_eq!(get(&v, "/records/0/active"), json!(true));
        assert_eq!(get(&v, "/records/1/score"), json!(7));
    }

    #[test]
    fn sniffs_semicolon_and_detects_no_header() {
        // all-numeric, first row is also numeric → no header
        let csv = "10;20;30\n40;50;60\n";
        let out = infer(csv, "auto", "auto", "", true, "schema").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(get(&v, "/dialect/delimiter_name"), json!("semicolon"));
        assert_eq!(get(&v, "/dialect/has_header"), json!(false));
        assert_eq!(get(&v, "/columns/0/name"), json!("column_1"));
        assert_eq!(get(&v, "/columns/0/type"), json!("int"));
        assert_eq!(get(&v, "/row_count"), json!(2));
        assert!(v.get("records").is_none());
    }

    #[test]
    fn leading_zero_codes_stay_strings() {
        let csv = "zip\n01234\n00755\n";
        let out = infer(csv, "auto", "present", "", true, "schema").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(get(&v, "/columns/0/type"), json!("string"));
    }

    #[test]
    fn null_tokens_and_nullable() {
        let csv = "a,b\n1,x\nNA,y\n3,\n";
        let out = infer(csv, "comma", "present", "NA", true, "both").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(get(&v, "/columns/0/type"), json!("int"));
        assert_eq!(get(&v, "/columns/0/nullable"), json!(true));
        assert_eq!(get(&v, "/columns/0/missing"), json!(1));
        assert_eq!(get(&v, "/records/1/a"), Value::Null);
        assert_eq!(get(&v, "/columns/1/missing"), json!(1));
    }

    #[test]
    fn datetime_and_quoted_fields() {
        let csv = "ts,note\n2024-01-31T09:15:00,\"hello, world\"\n2024-02-01T10:00:00,plain\n";
        let out = infer(csv, "auto", "auto", "", true, "both").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(get(&v, "/columns/0/type"), json!("datetime"));
        assert_eq!(get(&v, "/columns/0/format"), json!("YYYY-MM-DDTHH:MM:SS"));
        assert_eq!(get(&v, "/records/0/note"), json!("hello, world"));
    }

    #[test]
    fn records_only_output_is_an_array() {
        let csv = "n\n1\n2\n";
        let out = infer(csv, "comma", "present", "", true, "records").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(get(&v, "/0/n"), json!(1));
    }

    #[test]
    fn error_on_empty_input() {
        assert!(infer("   \n  ", "auto", "auto", "", true, "both").is_err());
    }

    #[test]
    fn error_on_bad_output() {
        let err = infer("a\n1\n", "auto", "auto", "", true, "bogus").unwrap_err();
        assert!(err.contains("unknown output"));
    }
}
