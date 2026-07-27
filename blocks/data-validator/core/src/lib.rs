//! data-validator core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Validate pasted CSV **or** JSON rows against a set of field rules and list
//! every violation with its record/line, field, offending value, the rule it
//! broke, and a human message. Report-only — the input is never modified and
//! nothing is fetched or persisted. A single linear pass over the records.
//!
//! Rules are written one-per-line (blank lines ignored) as
//! `field:rule` or `field:rule=argument`:
//!
//! ```text
//! email:required
//! email:regex=^[^@\s]+@[^@\s]+\.[^@\s]+$
//! age:type=int
//! age:min=18
//! age:max=120
//! plan:enum=free|pro|team
//! id:unique
//! ```
//!
//! Supported rules: `required`, `unique`, `type=<int|float|bool|date|email|url>`
//! (bare `age:int` is shorthand for `age:type=int`), `min=<n>`, `max=<n>`
//! (numeric range), `minlen=<n>`, `maxlen=<n>` (character-count range),
//! `regex=<pattern>` (unanchored by default — add `^…$` to anchor), and
//! `enum=a|b|c` (exact, case-sensitive membership). Every rule except
//! `required` is SKIPPED for a blank/missing value — combine with `required`
//! when a value must be present.

use regex::Regex;
use serde::Serialize;

/// Candidate CSV delimiters tried during auto-detection, in preference order.
const CANDIDATE_DELIMS: [(char, &str); 4] = [
    (',', "comma"),
    ('\t', "tab"),
    (';', "semicolon"),
    ('|', "pipe"),
];

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// One rule violation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Violation {
    /// 1-based record number (1 = first data record after the CSV header).
    pub record: usize,
    /// 1-based physical line where the record starts. For CSV this accounts for
    /// quoted newlines; for JSON it equals the record number (array element) or
    /// the source line (JSON Lines / NDJSON).
    pub line: usize,
    /// The field the rule targets (header name, `col N` for headerless CSV, or
    /// the JSON key).
    pub field: String,
    /// The offending value (empty string when the value is missing/blank).
    pub value: String,
    /// The rule that was broken, human form (e.g. `type=int`, `min=18`,
    /// `regex=^a$`, `enum(free|pro)`, `required`, `unique`).
    pub rule: String,
    /// Human-readable explanation of the violation.
    pub message: String,
}

/// The full validation report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True iff there are no violations AND every rule field exists in the data.
    pub valid: bool,
    /// Detected/declared input format: `csv` or `json`.
    pub input_format: String,
    /// For CSV: the resolved delimiter name. Empty for JSON.
    pub delimiter: String,
    /// True when the CSV delimiter was auto-detected rather than user-chosen.
    pub delimiter_detected: bool,
    /// Number of data records examined.
    pub records: usize,
    /// Number of rules parsed.
    pub rules: usize,
    /// Distinct fields that at least one rule targeted and that exist in the data.
    pub fields_checked: Vec<String>,
    /// Rule fields that do not exist in the data (typos / missing columns/keys).
    pub unknown_fields: Vec<String>,
    /// Total (record × resolved-rule) checks performed.
    pub checks_run: usize,
    /// Total violations found (NOT capped by max_issues).
    pub violation_count: usize,
    /// The violations, capped at max_issues.
    pub violations: Vec<Violation>,
    /// True when more violations were found than are listed.
    pub truncated: bool,
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeKind {
    Int,
    Float,
    Bool,
    Date,
    Email,
    Url,
}

impl TypeKind {
    fn name(&self) -> &'static str {
        match self {
            TypeKind::Int => "int",
            TypeKind::Float => "float",
            TypeKind::Bool => "bool",
            TypeKind::Date => "date",
            TypeKind::Email => "email",
            TypeKind::Url => "url",
        }
    }
    fn parse(s: &str) -> Option<TypeKind> {
        Some(match s {
            "int" | "integer" => TypeKind::Int,
            "float" | "number" | "decimal" | "double" => TypeKind::Float,
            "bool" | "boolean" => TypeKind::Bool,
            "date" => TypeKind::Date,
            "email" => TypeKind::Email,
            "url" => TypeKind::Url,
            _ => return None,
        })
    }
}

enum RuleKind {
    Required,
    Unique,
    Type(TypeKind),
    Min(f64),
    Max(f64),
    MinLen(usize),
    MaxLen(usize),
    Regex { pattern: String, re: Regex },
    Enum(Vec<String>),
}

impl RuleKind {
    /// Human form used in reports.
    fn human(&self) -> String {
        match self {
            RuleKind::Required => "required".to_string(),
            RuleKind::Unique => "unique".to_string(),
            RuleKind::Type(t) => format!("type={}", t.name()),
            RuleKind::Min(n) => format!("min={}", fmt_num(*n)),
            RuleKind::Max(n) => format!("max={}", fmt_num(*n)),
            RuleKind::MinLen(n) => format!("minlen={n}"),
            RuleKind::MaxLen(n) => format!("maxlen={n}"),
            RuleKind::Regex { pattern, .. } => format!("regex={pattern}"),
            RuleKind::Enum(vs) => format!("enum({})", vs.join("|")),
        }
    }
}

struct Rule {
    field: String,
    kind: RuleKind,
}

/// Format an f64 without a trailing `.0` when it is an integer value.
fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Parse the newline-separated rule list. Each non-blank line is `field:rule`
/// or `field:rule=arg`. Lines beginning with `#` are treated as comments.
fn parse_rules(rules: &str) -> Result<Vec<Rule>, String> {
    let mut out: Vec<Rule> = Vec::new();
    for raw_line in rules.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // field : rest — split on the FIRST colon (field names cannot contain ':').
        let colon = line.find(':').ok_or_else(|| {
            format!("rule '{line}' must be field:rule, e.g. age:type=int or email:required")
        })?;
        let field = line[..colon].trim();
        let rest = line[colon + 1..].trim();
        if field.is_empty() {
            return Err(format!("rule '{line}' is missing a field name before ':'"));
        }
        if rest.is_empty() {
            return Err(format!("rule '{line}' is missing a rule after ':'"));
        }
        // rule kind [= arg] — split on the FIRST '=' (regex/enum args may contain '=').
        let (kind_tok, arg): (String, Option<&str>) = match rest.find('=') {
            Some(eq) => (
                rest[..eq].trim().to_ascii_lowercase(),
                Some(rest[eq + 1..].trim()),
            ),
            None => (rest.to_ascii_lowercase(), None),
        };
        let kind = build_rule_kind(&kind_tok, arg, line)?;
        out.push(Rule {
            field: field.to_string(),
            kind,
        });
    }
    if out.is_empty() {
        return Err(
            "add at least one rule, e.g. `age:type=int` on its own line (see the examples)"
                .to_string(),
        );
    }
    Ok(out)
}

fn build_rule_kind(kind: &str, arg: Option<&str>, line: &str) -> Result<RuleKind, String> {
    // Rules with no argument.
    match kind {
        "required" => return no_arg(RuleKind::Required, arg, line),
        "unique" => return no_arg(RuleKind::Unique, arg, line),
        _ => {}
    }
    // Bare-type shorthand: `age:int` == `age:type=int`.
    if arg.is_none() {
        if let Some(t) = TypeKind::parse(kind) {
            return Ok(RuleKind::Type(t));
        }
        return Err(format!(
            "unknown rule '{kind}' in '{line}' — use required, unique, type=…, min=…, max=…, minlen=…, maxlen=…, regex=…, or enum=a|b|c"
        ));
    }
    let arg = arg.unwrap();
    match kind {
        "type" => {
            let t = TypeKind::parse(&arg.to_ascii_lowercase()).ok_or_else(|| {
                format!("unknown type '{arg}' in '{line}' — use int, float, bool, date, email, or url")
            })?;
            Ok(RuleKind::Type(t))
        }
        "min" => Ok(RuleKind::Min(parse_num_arg(arg, "min", line)?)),
        "max" => Ok(RuleKind::Max(parse_num_arg(arg, "max", line)?)),
        "minlen" => Ok(RuleKind::MinLen(parse_len_arg(arg, "minlen", line)?)),
        "maxlen" => Ok(RuleKind::MaxLen(parse_len_arg(arg, "maxlen", line)?)),
        "regex" | "pattern" | "match" => {
            if arg.is_empty() {
                return Err(format!("regex rule in '{line}' has an empty pattern"));
            }
            let re = Regex::new(arg)
                .map_err(|e| format!("invalid regex in '{line}': {}", first_line(&e.to_string())))?;
            Ok(RuleKind::Regex {
                pattern: arg.to_string(),
                re,
            })
        }
        "enum" | "in" | "oneof" => {
            let vals: Vec<String> = arg
                .split('|')
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect();
            if vals.is_empty() {
                return Err(format!(
                    "enum rule in '{line}' lists no values — write enum=a|b|c"
                ));
            }
            Ok(RuleKind::Enum(vals))
        }
        other => Err(format!(
            "unknown rule '{other}' in '{line}' — use required, unique, type=…, min=…, max=…, minlen=…, maxlen=…, regex=…, or enum=a|b|c"
        )),
    }
}

fn no_arg(kind: RuleKind, arg: Option<&str>, line: &str) -> Result<RuleKind, String> {
    if arg.is_some() {
        return Err(format!(
            "rule '{}' in '{line}' takes no '=' argument",
            kind.human()
        ));
    }
    Ok(kind)
}

fn parse_num_arg(arg: &str, name: &str, line: &str) -> Result<f64, String> {
    arg.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("{name} in '{line}' needs a number, got '{arg}'"))
}

fn parse_len_arg(arg: &str, name: &str, line: &str) -> Result<usize, String> {
    arg.trim()
        .parse::<usize>()
        .map_err(|_| format!("{name} in '{line}' needs a whole number ≥ 0, got '{arg}'"))
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).trim().to_string()
}

// ---------------------------------------------------------------------------
// Records (unified CSV + JSON representation)
// ---------------------------------------------------------------------------

/// A single data record, normalized to the known-field order. `values[i]` is the
/// value of `fields[i].0`, or `None` when that field is absent / JSON null.
struct Record {
    record: usize,
    line: usize,
    values: Vec<Option<String>>,
}

/// The parsed data: the known fields (internal name + display name) and records.
struct Dataset {
    /// (internal-name-used-in-rules, display-name-shown-in-reports)
    fields: Vec<(String, String)>,
    records: Vec<Record>,
    input_format: &'static str,
    delimiter: String,
    delimiter_detected: bool,
}

// --- CSV ---------------------------------------------------------------------

struct ParsedRow {
    line: usize,
    fields: Vec<String>,
}

/// RFC-4180-ish CSV parse: honors double-quoted fields (`""` → `"`, quoted
/// fields may span newlines), skips fully-blank physical lines, strips a BOM.
fn parse_csv(text: &str, delim: char) -> Vec<ParsedRow> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows: Vec<ParsedRow> = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut row_started = false;
    let mut line = 1usize;
    let mut row_line = 1usize;
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
            '\r' => {}
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
    if row_started || !field.is_empty() || !fields.is_empty() {
        fields.push(field);
        push_row(&mut rows, row_line, fields);
    }
    rows
}

fn push_row(rows: &mut Vec<ParsedRow>, line: usize, fields: Vec<String>) {
    if fields.len() == 1 && fields[0].is_empty() {
        return;
    }
    rows.push(ParsedRow { line, fields });
}

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

fn parse_csv_dataset(data: &str, header: bool, delimiter: &str) -> Result<Dataset, String> {
    let (delim, delim_name, detected) = resolve_delimiter(delimiter, data)?;
    let rows = parse_csv(data, delim);
    if rows.is_empty() {
        return Err("no CSV rows found in the data".to_string());
    }
    let (fields, data_start): (Vec<(String, String)>, usize) = if header {
        let names = rows[0]
            .fields
            .iter()
            .map(|h| {
                let t = h.trim().to_string();
                (t.clone(), t)
            })
            .collect();
        (names, 1usize)
    } else {
        let width = rows[0].fields.len();
        let names = (0..width)
            .map(|i| ((i + 1).to_string(), format!("col {}", i + 1)))
            .collect();
        (names, 0usize)
    };
    let width = fields.len();
    let mut records = Vec::new();
    for (ri, row) in rows.iter().enumerate().skip(data_start) {
        let mut values = Vec::with_capacity(width);
        for i in 0..width {
            values.push(row.fields.get(i).cloned());
        }
        records.push(Record {
            record: ri - data_start + 1,
            line: row.line,
            values,
        });
    }
    Ok(Dataset {
        fields,
        records,
        input_format: "csv",
        delimiter: delim_name.to_string(),
        delimiter_detected: detected,
    })
}

// --- JSON --------------------------------------------------------------------

/// Convert a JSON value into the string used for rule checks, or None when the
/// value should be treated as absent (JSON null).
fn json_value_to_string(v: &serde_json::Value) -> Option<String> {
    use serde_json::Value;
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        // Arrays/objects have no scalar form — stringify so type/regex checks
        // fail meaningfully rather than silently passing.
        other => Some(other.to_string()),
    }
}

fn json_type_name(v: &serde_json::Value) -> &'static str {
    use serde_json::Value;
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Parse `data` as a JSON array of objects, a single object, or JSON Lines
/// (one object per line). Returns the ordered union of keys + records.
fn parse_json_dataset(data: &str) -> Result<Dataset, String> {
    let trimmed = data.trim();
    // Collect (source_line, object) pairs.
    let mut objects: Vec<(usize, serde_json::Map<String, serde_json::Value>)> = Vec::new();

    let whole: Result<serde_json::Value, _> = serde_json::from_str(trimmed);
    match whole {
        Ok(serde_json::Value::Array(arr)) => {
            for (i, el) in arr.into_iter().enumerate() {
                match el {
                    serde_json::Value::Object(m) => objects.push((i + 1, m)),
                    other => {
                        return Err(format!(
                            "JSON array element {} is a {}, not an object — each row must be a JSON object",
                            i + 1,
                            json_type_name(&other)
                        ))
                    }
                }
            }
        }
        Ok(serde_json::Value::Object(m)) => objects.push((1, m)),
        Ok(other) => {
            return Err(format!(
                "JSON must be an array of objects, a single object, or JSON Lines — got a {}",
                json_type_name(&other)
            ))
        }
        Err(_) => {
            // Fall back to JSON Lines / NDJSON: one JSON object per non-blank line.
            for (idx, raw) in data.lines().enumerate() {
                let line_no = idx + 1;
                let l = raw.trim();
                if l.is_empty() {
                    continue;
                }
                let value: serde_json::Value = serde_json::from_str(l).map_err(|e| {
                    format!(
                        "line {line_no} is not valid JSON: {}",
                        first_line(&e.to_string())
                    )
                })?;
                match value {
                    serde_json::Value::Object(m) => objects.push((line_no, m)),
                    other => {
                        return Err(format!(
                            "line {line_no} is a {}, not an object — each JSON Lines row must be an object",
                            json_type_name(&other)
                        ))
                    }
                }
            }
        }
    }

    if objects.is_empty() {
        return Err("no JSON records found in the data".to_string());
    }

    // Ordered union of keys across all objects.
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_, m) in &objects {
        for k in m.keys() {
            if seen_keys.insert(k.clone()) {
                fields.push((k.clone(), k.clone()));
            }
        }
    }

    let mut records = Vec::with_capacity(objects.len());
    for (i, (line_no, m)) in objects.iter().enumerate() {
        let values = fields
            .iter()
            .map(|(k, _)| m.get(k).and_then(json_value_to_string))
            .collect();
        records.push(Record {
            record: i + 1,
            line: *line_no,
            values,
        });
    }

    Ok(Dataset {
        fields,
        records,
        input_format: "json",
        delimiter: String::new(),
        delimiter_detected: false,
    })
}

// ---------------------------------------------------------------------------
// Value checks (assume a non-empty, trimmed value)
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

/// ISO 8601 calendar date `YYYY-MM-DD`.
fn is_iso_date(s: &str) -> bool {
    let mut it = s.split('-');
    let (y, m, d) = match (it.next(), it.next(), it.next(), it.next()) {
        (Some(y), Some(m), Some(d), None) => (y, m, d),
        _ => return false,
    };
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return false;
    }
    let (yn, mn, dn) = match (y.parse::<i64>(), m.parse::<i64>(), d.parse::<i64>()) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        _ => return false,
    };
    (1..=12).contains(&mn) && dn >= 1 && dn <= days_in_month(yn, mn)
}

/// Small email syntax check (no I/O): `local@domain.tld`, single `@`, a dot in
/// the domain, no whitespace. Deliberately lenient — a syntax check, not a
/// deliverability check.
fn is_email(s: &str) -> bool {
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let (local, domain) = match s.split_once('@') {
        Some(p) => p,
        None => return false,
    };
    if local.is_empty() || domain.contains('@') {
        return false;
    }
    match domain.rsplit_once('.') {
        Some((label, tld)) => {
            !label.is_empty() && tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic())
        }
        None => false,
    }
}

/// http(s) URL syntax check (no I/O): `http://`|`https://` + a non-empty host
/// that contains a dot (or is `localhost`).
fn is_url(s: &str) -> bool {
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let rest = match s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
    {
        Some(r) => r,
        None => return false,
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    (!host.is_empty() && host.contains('.')) || host == "localhost"
}

/// Check one non-empty trimmed `value` against a per-value rule. Returns the
/// violation message, or None when it passes. `required`/`unique` are handled by
/// the caller (they have empty-value / cross-record semantics).
fn check_value(value: &str, kind: &RuleKind) -> Option<String> {
    match kind {
        RuleKind::Type(t) => {
            let ok = match t {
                TypeKind::Int => is_int(value),
                TypeKind::Float => is_float(value),
                TypeKind::Bool => is_bool(value),
                TypeKind::Date => is_iso_date(value),
                TypeKind::Email => is_email(value),
                TypeKind::Url => is_url(value),
            };
            if ok {
                None
            } else {
                Some(format!("\"{value}\" is not a valid {}", t.name()))
            }
        }
        RuleKind::Min(n) => match value.parse::<f64>().ok().filter(|x| x.is_finite()) {
            Some(x) if x >= *n => None,
            Some(x) => Some(format!("{} is less than min {}", fmt_num(x), fmt_num(*n))),
            None => Some(format!("\"{value}\" is not a number (min {})", fmt_num(*n))),
        },
        RuleKind::Max(n) => match value.parse::<f64>().ok().filter(|x| x.is_finite()) {
            Some(x) if x <= *n => None,
            Some(x) => Some(format!(
                "{} is greater than max {}",
                fmt_num(x),
                fmt_num(*n)
            )),
            None => Some(format!("\"{value}\" is not a number (max {})", fmt_num(*n))),
        },
        RuleKind::MinLen(n) => {
            let len = value.chars().count();
            if len >= *n {
                None
            } else {
                Some(format!("length {len} is under minlen {n}"))
            }
        }
        RuleKind::MaxLen(n) => {
            let len = value.chars().count();
            if len <= *n {
                None
            } else {
                Some(format!("length {len} is over maxlen {n}"))
            }
        }
        RuleKind::Regex { pattern, re } => {
            if re.is_match(value) {
                None
            } else {
                Some(format!("\"{value}\" does not match regex {pattern}"))
            }
        }
        RuleKind::Enum(vals) => {
            if vals.iter().any(|v| v == value) {
                None
            } else {
                Some(format!("\"{value}\" is not one of {}", vals.join(", ")))
            }
        }
        // Handled by the caller.
        RuleKind::Required | RuleKind::Unique => None,
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

fn resolve_format(spec: &str, data: &str) -> Result<&'static str, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => {
            let t = data.strip_prefix('\u{feff}').unwrap_or(data).trim_start();
            Ok(if t.starts_with('[') || t.starts_with('{') {
                "json"
            } else {
                "csv"
            })
        }
        "csv" => Ok("csv"),
        "json" | "ndjson" | "jsonl" => Ok("json"),
        other => Err(format!(
            "unknown input_format '{other}' — use auto, csv, or json"
        )),
    }
}

/// Validate `data` (CSV or JSON) against `rules` and return a structured report.
pub fn validate(
    data: &str,
    rules: &str,
    input_format: &str,
    header: bool,
    delimiter: &str,
    max_issues: usize,
) -> Result<Report, String> {
    if data.trim().is_empty() {
        return Err("no data — paste CSV or JSON rows to validate".to_string());
    }
    let max_issues = max_issues.clamp(1, 1000);
    let parsed_rules = parse_rules(rules)?;

    let fmt = resolve_format(input_format, data)?;
    let ds = match fmt {
        "json" => parse_json_dataset(data)?,
        _ => parse_csv_dataset(data, header, delimiter)?,
    };

    // Resolve each rule's field to a known field position (or record it unknown).
    // Field names match case-sensitively after trimming.
    let mut resolved: Vec<(usize, String, &Rule)> = Vec::new();
    let mut unknown_fields: Vec<String> = Vec::new();
    let mut fields_checked: Vec<String> = Vec::new();
    for rule in &parsed_rules {
        let key = rule.field.trim();
        match ds.fields.iter().position(|(internal, _)| internal == key) {
            Some(pos) => {
                let display = ds.fields[pos].1.clone();
                if !fields_checked.contains(&display) {
                    fields_checked.push(display.clone());
                }
                resolved.push((pos, display, rule));
            }
            None => {
                if !unknown_fields.contains(&rule.field) {
                    unknown_fields.push(rule.field.clone());
                }
            }
        }
    }

    // Per-resolved-rule "already seen" maps for `unique`.
    let mut unique_seen: Vec<std::collections::HashMap<String, usize>> =
        (0..resolved.len()).map(|_| Default::default()).collect();

    let mut violations: Vec<Violation> = Vec::new();
    let mut violation_count = 0usize;
    let mut checks_run = 0usize;

    for rec in &ds.records {
        for (ri, (pos, display, rule)) in resolved.iter().enumerate() {
            checks_run += 1;
            let raw = rec.values.get(*pos).and_then(|v| v.as_ref());
            let trimmed = raw.map(|s| s.trim()).unwrap_or("");
            let empty = trimmed.is_empty();

            let message: Option<String> = match &rule.kind {
                RuleKind::Required => {
                    if empty {
                        Some("missing required value".to_string())
                    } else {
                        None
                    }
                }
                RuleKind::Unique => {
                    if empty {
                        None
                    } else {
                        match unique_seen[ri].get(trimmed) {
                            Some(first) => {
                                Some(format!("duplicate value (already seen at record {first})"))
                            }
                            None => {
                                unique_seen[ri].insert(trimmed.to_string(), rec.record);
                                None
                            }
                        }
                    }
                }
                _ => {
                    if empty {
                        None
                    } else {
                        check_value(trimmed, &rule.kind)
                    }
                }
            };

            if let Some(message) = message {
                violation_count += 1;
                if violations.len() < max_issues {
                    violations.push(Violation {
                        record: rec.record,
                        line: rec.line,
                        field: display.clone(),
                        value: raw.cloned().unwrap_or_default(),
                        rule: rule.kind.human(),
                        message,
                    });
                }
            }
        }
    }

    let truncated = violation_count > violations.len();
    let valid = violation_count == 0 && unknown_fields.is_empty();

    Ok(Report {
        valid,
        input_format: ds.input_format.to_string(),
        delimiter: ds.delimiter,
        delimiter_detected: ds.delimiter_detected,
        records: ds.records.len(),
        rules: parsed_rules.len(),
        fields_checked,
        unknown_fields,
        checks_run,
        violation_count,
        violations,
        truncated,
    })
}

/// Human-readable report for the page / chat (`format = "text"`).
pub fn summary(
    data: &str,
    rules: &str,
    input_format: &str,
    header: bool,
    delimiter: &str,
    max_issues: usize,
) -> Result<String, String> {
    let r = validate(data, rules, input_format, header, delimiter, max_issues)?;

    let verdict = if r.valid {
        if r.checks_run == 0 {
            "Valid — no checks to run.".to_string()
        } else {
            format!(
                "Valid — all {} check(s) passed across {} record(s).",
                r.checks_run, r.records
            )
        }
    } else if r.violation_count == 0 {
        "INVALID — rule field(s) not found in the data.".to_string()
    } else {
        format!(
            "INVALID — {} violation(s) across {} record(s).",
            r.violation_count, r.records
        )
    };

    let source = if r.input_format == "csv" {
        format!(
            "CSV · delimiter {}{}",
            r.delimiter,
            if r.delimiter_detected {
                " (auto-detected)"
            } else {
                ""
            }
        )
    } else {
        "JSON".to_string()
    };

    let checked = if r.fields_checked.is_empty() {
        "none".to_string()
    } else {
        r.fields_checked.join(", ")
    };

    let mut out = format!(
        "{verdict}\n{source} · {} record(s) · {} rule(s) · {} check(s)\nFields checked: {}\n",
        r.records, r.rules, r.checks_run, checked
    );

    if !r.unknown_fields.is_empty() {
        out.push_str(&format!(
            "Rule field(s) not found: {}\n",
            r.unknown_fields.join(", ")
        ));
    }

    for v in &r.violations {
        out.push_str(&format!(
            "Record {} (line {}), field \"{}\" [{}] — {}\n",
            v.record, v.line, v.field, v.rule, v.message
        ));
    }

    if r.truncated {
        let hidden = r.violation_count - r.violations.len();
        out.push_str(&format!(
            "(+ {hidden} more violation(s) not shown — raise max_issues to list them)\n"
        ));
    }

    Ok(out.trim_end().to_string())
}

/// Validate and render either a human report (`text`) or the JSON report.
pub fn run(
    data: &str,
    rules: &str,
    input_format: &str,
    header: bool,
    delimiter: &str,
    max_issues: usize,
    format: &str,
) -> Result<String, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => summary(data, rules, input_format, header, delimiter, max_issues),
        "json" => {
            let report = validate(data, rules, input_format, header, delimiter, max_issues)?;
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

    fn v(data: &str, rules: &str) -> Report {
        validate(data, rules, "auto", true, "auto", 50).unwrap()
    }

    #[test]
    fn clean_csv_passes_all_rule_kinds() {
        let data = "email,age,plan,code\n\
                    a@b.com,34,pro,AB12\n\
                    c@d.org,7,free,ZZ99\n";
        let rules = "email:required\n\
                     email:type=email\n\
                     age:type=int\n\
                     age:min=0\n\
                     age:max=120\n\
                     plan:enum=free|pro|team\n\
                     code:regex=^[A-Z]{2}\\d{2}$\n\
                     email:unique";
        let r = v(data, rules);
        assert!(r.valid, "expected valid, got {:?}", r.violations);
        assert_eq!(r.records, 2);
        assert_eq!(r.input_format, "csv");
        assert_eq!(r.delimiter, "comma");
        assert!(r.delimiter_detected);
        assert_eq!(r.violation_count, 0);
    }

    #[test]
    fn flags_each_rule_kind() {
        let data = "email,age,plan\n\
                    not-an-email,200,gold\n";
        let rules = "email:type=email\nage:max=120\nplan:enum=free|pro";
        let r = v(data, rules);
        assert!(!r.valid);
        assert_eq!(r.violation_count, 3);
        let rules_hit: Vec<&str> = r.violations.iter().map(|x| x.rule.as_str()).collect();
        assert!(rules_hit.contains(&"type=email"));
        assert!(rules_hit.contains(&"max=120"));
        assert!(rules_hit.contains(&"enum(free|pro)"));
        assert!(r.violations.iter().all(|x| x.record == 1));
    }

    #[test]
    fn required_flags_missing_and_blank() {
        let data = "name,email\nAda,\nBo,b@x.com\n";
        let r = v(data, "email:required");
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].record, 1);
        assert_eq!(r.violations[0].message, "missing required value");
    }

    #[test]
    fn constraints_skip_blank_values() {
        // age blank on record 1 — type/min do NOT fire without `required`.
        let data = "age,note\n,blank age\n30,ok\n";
        let r = v(data, "age:type=int\nage:min=18");
        assert!(r.valid, "blank should be skipped: {:?}", r.violations);
        assert_eq!(r.records, 2);
    }

    #[test]
    fn unique_flags_second_occurrence() {
        let data = "id\n1\n2\n1\n";
        let r = v(data, "id:unique");
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].record, 3);
        assert!(r.violations[0].message.contains("record 1"));
    }

    #[test]
    fn min_max_numeric() {
        let data = "n\n5\n50\n";
        let r = v(data, "n:min=10\nn:max=40");
        assert_eq!(r.violation_count, 2);
        assert!(r
            .violations
            .iter()
            .any(|x| x.record == 1 && x.rule == "min=10"));
        assert!(r
            .violations
            .iter()
            .any(|x| x.record == 2 && x.rule == "max=40"));
    }

    #[test]
    fn min_on_non_number_flags() {
        let data = "n\nabc\n";
        let r = v(data, "n:min=1");
        assert_eq!(r.violation_count, 1);
        assert!(r.violations[0].message.contains("not a number"));
    }

    #[test]
    fn len_rules() {
        let data = "pw\nabc\nabcdefghij\n";
        let r = v(data, "pw:minlen=4\npw:maxlen=8");
        assert_eq!(r.violation_count, 2);
    }

    #[test]
    fn bare_type_shorthand() {
        let data = "age\nx\n";
        let r = v(data, "age:int");
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].rule, "type=int");
    }

    #[test]
    fn unknown_field_invalidates() {
        let data = "name\nAda\n";
        let r = v(data, "emial:required");
        assert!(!r.valid);
        assert_eq!(r.violation_count, 0);
        assert_eq!(r.unknown_fields, vec!["emial".to_string()]);
    }

    #[test]
    fn json_array_of_objects() {
        let data = r#"[{"id": 1, "email": "a@b.com"}, {"id": 2, "email": "bad"}]"#;
        let r = v(data, "email:type=email\nid:type=int");
        assert_eq!(r.input_format, "json");
        assert_eq!(r.records, 2);
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].record, 2);
        assert_eq!(r.violations[0].field, "email");
    }

    #[test]
    fn json_numbers_check_min_max_natively() {
        let data = r#"[{"age": 5}, {"age": 200}]"#;
        let r = v(data, "age:min=18\nage:max=120");
        assert_eq!(r.violation_count, 2);
    }

    #[test]
    fn json_null_is_absent_for_required() {
        let data = r#"[{"email": null}, {"email": "a@b.com"}]"#;
        let r = v(data, "email:required");
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].record, 1);
    }

    #[test]
    fn json_lines_ndjson() {
        let data = "{\"n\": 1}\n{\"n\": \"oops\"}\n";
        let r = validate(data, "n:type=int", "json", true, "auto", 50).unwrap();
        assert_eq!(r.records, 2);
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].line, 2);
    }

    #[test]
    fn headerless_csv_indices() {
        let data = "Ada,34\nBo,x\n";
        let r = validate(data, "2:type=int", "csv", false, "comma", 50).unwrap();
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].field, "col 2");
        assert_eq!(r.violations[0].value, "x");
    }

    #[test]
    fn max_issues_truncates() {
        let mut data = String::from("n\n");
        for _ in 0..10 {
            data.push_str("x\n");
        }
        let r = validate(&data, "n:type=int", "csv", true, "comma", 3).unwrap();
        assert_eq!(r.violation_count, 10);
        assert_eq!(r.violations.len(), 3);
        assert!(r.truncated);
    }

    #[test]
    fn quoted_field_with_delimiter() {
        let data = "name,email\n\"Smith, John\",bad\n";
        let r = v(data, "email:type=email");
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].value, "bad");
    }

    #[test]
    fn regex_anchoring() {
        let data = "code\nAB12\nabc\n";
        let r = v(data, "code:regex=^[A-Z]{2}\\d{2}$");
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].value, "abc");
    }

    #[test]
    fn iso_date_calendar_check() {
        let data = "d\n2021-02-29\n2020-02-29\n";
        let r = v(data, "d:type=date");
        // 2021 not leap → invalid; 2020 leap → valid
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].value, "2021-02-29");
    }

    #[test]
    fn url_check() {
        let data = "site\nhttps://example.com/x\nexample.com\n";
        let r = v(data, "site:type=url");
        assert_eq!(r.violation_count, 1);
        assert_eq!(r.violations[0].value, "example.com");
    }

    #[test]
    fn invalid_regex_errors() {
        let err = validate("a\n1\n", "a:regex=[unclosed", "csv", true, "comma", 50).unwrap_err();
        assert!(err.contains("invalid regex"), "got: {err}");
    }

    #[test]
    fn bad_rule_errors() {
        let err = validate("a\n1\n", "a:widget", "csv", true, "comma", 50).unwrap_err();
        assert!(err.contains("unknown rule"), "got: {err}");
    }

    #[test]
    fn empty_rules_errors() {
        let err = validate("a\n1\n", "", "csv", true, "comma", 50).unwrap_err();
        assert!(err.contains("add at least one rule"), "got: {err}");
    }

    #[test]
    fn missing_colon_errors() {
        let err = validate("a\n1\n", "arequired", "csv", true, "comma", 50).unwrap_err();
        assert!(err.contains("must be field:rule"), "got: {err}");
    }

    #[test]
    fn comment_and_blank_lines_ignored() {
        let data = "n\n1\n";
        let r = v(data, "# this is a comment\n\nn:type=int\n");
        assert!(r.valid);
        assert_eq!(r.rules, 1);
    }

    #[test]
    fn summary_renders_violations() {
        let data = "email,age\na@b.com,200\n";
        let s = summary(data, "age:max=120", "auto", true, "auto", 50).unwrap();
        assert!(s.starts_with("INVALID — 1 violation(s)"), "got: {s}");
        assert!(s.contains("Record 1 (line 2), field \"age\" [max=120]"));
    }

    #[test]
    fn summary_valid() {
        let s = summary("n\n5\n", "n:type=int", "auto", true, "auto", 50).unwrap();
        assert!(s.starts_with("Valid — all 1 check(s) passed"), "got: {s}");
    }

    #[test]
    fn run_json_output() {
        let out = run(
            "email\nbad\n",
            "email:type=email",
            "csv",
            true,
            "comma",
            50,
            "json",
        )
        .unwrap();
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["valid"], false);
        assert_eq!(j["violation_count"], 1);
        assert_eq!(j["violations"][0]["field"], "email");
        assert_eq!(j["input_format"], "csv");
    }

    #[test]
    fn run_rejects_unknown_format() {
        let err = run("a\n1\n", "a:int", "csv", true, "comma", 50, "xml").unwrap_err();
        assert!(err.contains("unknown format"));
    }

    #[test]
    fn numeric_helpers() {
        assert!(is_int("-42"));
        assert!(!is_int("3.0"));
        assert!(is_float("3.14"));
        assert!(is_float("-.5"));
        assert!(is_float("6.02E23"));
        assert!(!is_float("1.2.3"));
        assert!(is_bool("YES"));
        assert!(!is_bool("maybe"));
        assert!(is_email("a.b@c.co"));
        assert!(!is_email("a@b"));
        assert!(is_url("http://x.io"));
        assert!(!is_url("ftp://x.io"));
    }
}
