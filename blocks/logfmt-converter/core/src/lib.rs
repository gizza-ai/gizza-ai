//! Pure conversion engine for gizza-ai/logfmt-converter.
//!
//! Converts pasted log/event data between **logfmt**, **JSON**, **NDJSON** and
//! **CSV** in either direction. Everything funnels through one intermediate
//! model — an ordered list of records, each an insertion-ordered map of
//! `key -> serde_json::Value` — so any source format can be written back out as
//! any target format.
//!
//! What makes this block distinct from the repo's other log tools is the logfmt
//! **writer**: `log-parser` and `text-to-json` only ever read logfmt, and
//! `data-format-converter` does not know logfmt at all.
//!
//! Encoding follows the de-facto go-logfmt rules: `key=value` pairs separated by
//! a single space, one record per line, values double-quoted whenever they hold
//! a space, `=`, a quote or a control character, with `\" \\ \n \r \t` escapes.
//! Two deliberate disambiguations on top of that:
//!   * `null` writes as a bare `key=` and an empty string as `key=""`, so a
//!     round-trip through logfmt with `detect_types` on preserves the two;
//!   * keys containing whitespace, `=` or quotes are sanitised to `_` rather
//!     than dropping the pair.
//!
//! Limits: 1,000,000 characters of input per run (checked before parsing).

use serde_json::{Map, Value};

/// Maximum accepted input size, in characters (not bytes).
pub const MAX_INPUT_CHARS: usize = 1_000_000;

/// One parsed record: an ordered key -> value map.
type Record = Vec<(String, Value)>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fmt {
    Logfmt,
    Json,
    Ndjson,
    Csv,
}

impl Fmt {
    fn parse_from(s: &str) -> Result<Option<Fmt>, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(None),
            "logfmt" => Ok(Some(Fmt::Logfmt)),
            "json" => Ok(Some(Fmt::Json)),
            "ndjson" | "jsonl" | "json-lines" => Ok(Some(Fmt::Ndjson)),
            "csv" => Ok(Some(Fmt::Csv)),
            other => Err(format!(
                "unknown from format '{other}': expected auto, logfmt, json, ndjson, or csv"
            )),
        }
    }

    fn parse_to(s: &str) -> Result<Fmt, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(Fmt::Json),
            "logfmt" => Ok(Fmt::Logfmt),
            "ndjson" | "jsonl" | "json-lines" => Ok(Fmt::Ndjson),
            "csv" => Ok(Fmt::Csv),
            other => Err(format!(
                "unknown to format '{other}': expected logfmt, json, ndjson, or csv"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Fmt::Logfmt => "logfmt",
            Fmt::Json => "json",
            Fmt::Ndjson => "ndjson",
            Fmt::Csv => "csv",
        }
    }
}

fn parse_delimiter(s: &str) -> Result<char, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "comma" | "," => Ok(','),
        "semicolon" | ";" => Ok(';'),
        "tab" | "\t" => Ok('\t'),
        "pipe" | "|" => Ok('|'),
        other => Err(format!(
            "unknown delimiter '{other}': expected comma, semicolon, tab, or pipe"
        )),
    }
}

/// Convert `data` from one log/event format to another.
///
/// * `from` — `auto` (default) | `logfmt` | `json` | `ndjson` | `csv`.
/// * `to` — `logfmt` | `json` (default) | `ndjson` | `csv`.
/// * `delimiter` — `comma` (default) | `semicolon` | `tab` | `pipe`, for CSV on
///   both sides.
/// * `detect_types` — coerce unquoted logfmt/CSV values to numbers, booleans and
///   null.
/// * `pretty` — indent JSON output (`to=json` only).
/// * `flatten` — flatten nested objects/arrays into dot-notation keys when
///   writing the flat formats (logfmt, CSV).
/// * `keys` — comma-separated allow-list that also fixes output field order;
///   blank keeps every key in first-seen order.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    data: &str,
    from: &str,
    to: &str,
    delimiter: &str,
    detect_types: bool,
    pretty: bool,
    flatten: bool,
    keys: &str,
) -> Result<String, String> {
    let chars = data.chars().count();
    if chars > MAX_INPUT_CHARS {
        return Err(format!(
            "input is {chars} characters; the maximum is {MAX_INPUT_CHARS}"
        ));
    }
    if data.trim().is_empty() {
        return Err("no input: paste some logfmt, JSON, NDJSON, or CSV text to convert".into());
    }

    let to_fmt = Fmt::parse_to(to)?;
    let delim = parse_delimiter(delimiter)?;
    let from_fmt = match Fmt::parse_from(from)? {
        Some(f) => f,
        None => detect(data, delim).ok_or_else(|| {
            "could not auto-detect the input format — set from to logfmt, json, ndjson, or csv"
                .to_string()
        })?,
    };

    let mut records = match from_fmt {
        Fmt::Logfmt => parse_logfmt(data, detect_types),
        Fmt::Json => parse_json(data)?,
        Fmt::Ndjson => parse_ndjson(data)?,
        Fmt::Csv => parse_csv(data, delim, detect_types)?,
    };

    if records.is_empty() {
        return Err(format!(
            "no records found: the input parsed as {} but produced no rows",
            from_fmt.name()
        ));
    }

    let select = parse_key_list(keys);
    if let Some(sel) = &select {
        records = records
            .into_iter()
            .map(|rec| project(rec, sel))
            .collect::<Vec<_>>();
    }

    Ok(match to_fmt {
        Fmt::Logfmt => write_logfmt(&records, flatten),
        Fmt::Json => write_json(&records, pretty),
        Fmt::Ndjson => write_ndjson(&records),
        Fmt::Csv => write_csv(&records, delim, flatten, select.as_deref()),
    })
}

fn parse_key_list(keys: &str) -> Option<Vec<String>> {
    let list: Vec<String> = keys
        .split(',')
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

/// Keep only the selected keys, in the selected order. A record missing one of
/// them simply omits it (CSV still emits the column, empty).
fn project(rec: Record, sel: &[String]) -> Record {
    let mut out = Vec::with_capacity(sel.len());
    for want in sel {
        if let Some((k, v)) = rec.iter().find(|(k, _)| k == want) {
            out.push((k.clone(), v.clone()));
        }
    }
    out
}

// ---------------------------------------------------------------- detection

fn data_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect()
}

/// Detect the input format. JSON/NDJSON are recognised by their punctuation,
/// logfmt by a leading `key=` token, and CSV last (it is the least specific).
fn detect(text: &str, delim: char) -> Option<Fmt> {
    let lines = data_lines(text);
    let first = lines.first()?.trim_start();
    if first.starts_with('[') {
        return Some(Fmt::Json);
    }
    if first.starts_with('{') {
        // Every non-blank line its own JSON value => JSON-Lines, else one
        // (possibly multi-line) JSON document.
        if lines.len() > 1
            && lines
                .iter()
                .all(|l| serde_json::from_str::<Value>(l.trim()).is_ok())
        {
            return Some(Fmt::Ndjson);
        }
        return Some(Fmt::Json);
    }
    if lines.iter().any(|l| logfmt_pair_count(l) >= 1) && logfmt_pair_count(first) >= 1 {
        return Some(Fmt::Logfmt);
    }
    if first.contains(delim) {
        return Some(Fmt::Csv);
    }
    None
}

/// How many `key=value` tokens a line starts with (used only for detection).
fn logfmt_pair_count(line: &str) -> usize {
    logfmt_pairs(line)
        .iter()
        .filter(|(k, v, _)| !k.is_empty() && v.is_some())
        .count()
}

// ------------------------------------------------------------------ logfmt

/// Tokenize one logfmt line into `(key, Option<value>, value_was_quoted)`.
/// A bare token with no `=` is a boolean flag; `key=` is an explicit empty.
fn logfmt_pairs(line: &str) -> Vec<(String, Option<String>, bool)> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        // Key: everything up to '=' or whitespace.
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if i < chars.len() && chars[i] == '=' {
            i += 1; // consume '='
            if i < chars.len() && chars[i] == '"' {
                let (val, next) = read_quoted(&chars, i);
                out.push((key, Some(val), true));
                i = next;
            } else {
                let vstart = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                out.push((key, Some(chars[vstart..i].iter().collect()), false));
            }
        } else if !key.is_empty() {
            out.push((key, None, false));
        }
    }
    out
}

/// Read a double-quoted logfmt value starting at `chars[i] == '"'`; returns the
/// unescaped content and the index just past the closing quote.
fn read_quoted(chars: &[char], i: usize) -> (String, usize) {
    let mut out = String::new();
    let mut j = i + 1;
    while j < chars.len() {
        match chars[j] {
            '"' => {
                j += 1;
                return (out, j);
            }
            '\\' if j + 1 < chars.len() => {
                let esc = chars[j + 1];
                out.push(match esc {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                });
                j += 2;
            }
            c => {
                out.push(c);
                j += 1;
            }
        }
    }
    // Unterminated quote: take the rest of the line.
    (out, j)
}

fn parse_logfmt(text: &str, detect_types: bool) -> Vec<Record> {
    let mut records = Vec::new();
    for line in data_lines(text) {
        let mut rec: Record = Vec::new();
        for (key, val, quoted) in logfmt_pairs(line) {
            if key.is_empty() {
                continue;
            }
            let value = match val {
                // A bare key is a logfmt boolean flag.
                None => Value::Bool(true),
                Some(raw) => coerce(&raw, quoted, detect_types),
            };
            upsert(&mut rec, key, value);
        }
        if !rec.is_empty() {
            records.push(rec);
        }
    }
    records
}

/// Later duplicate keys on one line win, keeping the first position.
fn upsert(rec: &mut Record, key: String, value: Value) {
    if let Some(slot) = rec.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = value;
    } else {
        rec.push((key, value));
    }
}

/// Unquoted scalars become numbers/booleans/null when `detect` is on; quoted
/// values always stay strings so `level="200"` survives as text.
fn coerce(raw: &str, quoted: bool, detect: bool) -> Value {
    if quoted || !detect {
        return Value::String(raw.to_string());
    }
    match raw {
        "" => Value::Null,
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => {
            // Keep leading zeros / '+' prefixes as text (zip codes, ids).
            let looks_padded = (raw.len() > 1 && raw.starts_with('0') && !raw.starts_with("0."))
                || raw.starts_with('+');
            if !looks_padded {
                if let Ok(n) = raw.parse::<i64>() {
                    return Value::from(n);
                }
                if let Ok(f) = raw.parse::<f64>() {
                    if f.is_finite() {
                        if let Some(n) = serde_json::Number::from_f64(f) {
                            return Value::Number(n);
                        }
                    }
                }
            }
            Value::String(raw.to_string())
        }
    }
}

// -------------------------------------------------------------- JSON/NDJSON

fn object_to_record(map: &Map<String, Value>) -> Record {
    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn value_to_record(v: &Value, where_: &str) -> Result<Record, String> {
    match v {
        Value::Object(m) => Ok(object_to_record(m)),
        _ => Err(format!(
            "{where_} is {}, but records must be JSON objects",
            type_name(v)
        )),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn parse_json(text: &str) -> Result<Vec<Record>, String> {
    let v: Value =
        serde_json::from_str(text.trim()).map_err(|e| format!("invalid JSON input: {e}"))?;
    match v {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, item)| value_to_record(item, &format!("array item {}", i + 1)))
            .collect(),
        Value::Object(m) => Ok(vec![object_to_record(&m)]),
        other => Err(format!(
            "JSON input is {}, but must be an object or an array of objects",
            type_name(&other)
        )),
    }
}

fn parse_ndjson(text: &str) -> Result<Vec<Record>, String> {
    let mut out = Vec::new();
    for (i, line) in data_lines(text).iter().enumerate() {
        let v: Value = serde_json::from_str(line.trim())
            .map_err(|e| format!("invalid JSON on line {}: {e}", i + 1))?;
        out.push(value_to_record(&v, &format!("line {}", i + 1))?);
    }
    Ok(out)
}

// ------------------------------------------------------------------- CSV

/// Split one delimited line, reporting whether each field was quoted.
fn split_delimited(line: &str, delim: char) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted_field = false;
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            cur.clear();
            in_quotes = true;
            quoted_field = true;
        } else if c == delim {
            out.push((cur.trim().to_string(), quoted_field));
            cur = String::new();
            quoted_field = false;
        } else {
            cur.push(c);
        }
    }
    out.push((cur.trim().to_string(), quoted_field));
    out
}

fn parse_csv(text: &str, delim: char, detect_types: bool) -> Result<Vec<Record>, String> {
    let lines = data_lines(text);
    let header: Vec<String> = split_delimited(lines[0], delim)
        .into_iter()
        .enumerate()
        .map(|(i, (name, _))| {
            if name.is_empty() {
                format!("column{}", i + 1)
            } else {
                name
            }
        })
        .collect();
    if lines.len() < 2 {
        return Err(
            "CSV input has only a header row — add at least one data row, or set from=logfmt/json if that is what you pasted"
                .into(),
        );
    }
    let mut out = Vec::new();
    for line in &lines[1..] {
        let cells = split_delimited(line, delim);
        let mut rec: Record = Vec::new();
        for (i, name) in header.iter().enumerate() {
            let (raw, quoted) = cells
                .get(i)
                .cloned()
                .unwrap_or_else(|| (String::new(), false));
            upsert(&mut rec, name.clone(), coerce(&raw, quoted, detect_types));
        }
        out.push(rec);
    }
    Ok(out)
}

// ----------------------------------------------------------------- writers

/// Render a value for a FLAT target (logfmt / CSV cell). Nested values are
/// either flattened by the caller or serialised compactly here.
fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Expand nested objects/arrays into dot-notation keys (`addr.city`, `tags.0`).
fn flatten_into(prefix: &str, v: &Value, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, sub) in m {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_into(&key, sub, out);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (i, sub) in items.iter().enumerate() {
                let key = if prefix.is_empty() {
                    i.to_string()
                } else {
                    format!("{prefix}.{i}")
                };
                flatten_into(&key, sub, out);
            }
        }
        other => out.push((prefix.to_string(), other.clone())),
    }
}

fn flat_pairs(rec: &Record, flatten: bool) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for (k, v) in rec {
        match v {
            Value::Object(_) | Value::Array(_) if flatten => flatten_into(k, v, &mut out),
            _ => out.push((k.clone(), v.clone())),
        }
    }
    out
}

/// logfmt keys cannot contain whitespace, `=` or quotes — sanitise instead of
/// dropping the pair.
fn sanitize_key(k: &str) -> String {
    let cleaned: String = k
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '=' || c == '"' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    if cleaned.is_empty() {
        "_".into()
    } else {
        cleaned
    }
}

fn needs_quotes(s: &str) -> bool {
    s.is_empty()
        || s.chars()
            .any(|c| c.is_whitespace() || c == '=' || c == '"' || c.is_control())
}

fn quote_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_logfmt(records: &[Record], flatten: bool) -> String {
    let mut lines = Vec::with_capacity(records.len());
    for rec in records {
        let mut parts = Vec::new();
        for (k, v) in flat_pairs(rec, flatten) {
            // `null` -> bare `key=`; empty string -> `key=""` (round-trips).
            let rendered = if v.is_null() {
                String::new()
            } else {
                let text = scalar_text(&v);
                if needs_quotes(&text) {
                    quote_value(&text)
                } else {
                    text
                }
            };
            parts.push(format!("{}={}", sanitize_key(&k), rendered));
        }
        lines.push(parts.join(" "));
    }
    lines.join("\n")
}

fn record_to_value(rec: &Record) -> Value {
    let mut map = Map::new();
    for (k, v) in rec {
        map.insert(k.clone(), v.clone());
    }
    Value::Object(map)
}

fn write_json(records: &[Record], pretty: bool) -> String {
    let arr = Value::Array(records.iter().map(record_to_value).collect());
    if pretty {
        serde_json::to_string_pretty(&arr).unwrap_or_default()
    } else {
        serde_json::to_string(&arr).unwrap_or_default()
    }
}

fn write_ndjson(records: &[Record]) -> String {
    records
        .iter()
        .map(|r| serde_json::to_string(&record_to_value(r)).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn csv_escape(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn write_csv(records: &[Record], delim: char, flatten: bool, select: Option<&[String]>) -> String {
    let flat: Vec<Vec<(String, Value)>> = records.iter().map(|r| flat_pairs(r, flatten)).collect();
    // Column order: the explicit selection, else union of keys in first-seen order.
    let mut header: Vec<String> = Vec::new();
    match select {
        Some(sel) if !flatten => header = sel.to_vec(),
        _ => {
            for rec in &flat {
                for (k, _) in rec {
                    if !header.iter().any(|h| h == k) {
                        header.push(k.clone());
                    }
                }
            }
        }
    }
    let mut lines = Vec::with_capacity(flat.len() + 1);
    lines.push(
        header
            .iter()
            .map(|h| csv_escape(h, delim))
            .collect::<Vec<_>>()
            .join(&delim.to_string()),
    );
    for rec in &flat {
        let row: Vec<String> = header
            .iter()
            .map(|h| {
                rec.iter()
                    .find(|(k, _)| k == h)
                    .map(|(_, v)| csv_escape(&scalar_text(v), delim))
                    .unwrap_or_default()
            })
            .collect();
        lines.push(row.join(&delim.to_string()));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &str, from: &str, to: &str) -> Result<String, String> {
        convert(data, from, to, "comma", true, false, true, "")
    }

    #[test]
    fn logfmt_to_json_types_and_quotes() {
        let out = run(
            r#"level=info msg="user signed in" ok=true count=3 ratio=0.5"#,
            "auto",
            "json",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"level":"info","msg":"user signed in","ok":true,"count":3,"ratio":0.5}]"#
        );
    }

    #[test]
    fn json_to_logfmt_quotes_and_escapes() {
        let out = run(
            r#"[{"level":"error","msg":"disk \"sda\" full","retries":2,"note":null,"empty":""}]"#,
            "json",
            "logfmt",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"level=error msg="disk \"sda\" full" retries=2 note= empty="""#
        );
    }

    #[test]
    fn logfmt_round_trip_preserves_null_vs_empty() {
        let src = r#"a= b="" c=x"#;
        let json = run(src, "logfmt", "json").unwrap();
        assert_eq!(json, r#"[{"a":null,"b":"","c":"x"}]"#);
        let back = run(&json, "json", "logfmt").unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn bare_key_is_a_boolean_flag() {
        let out = run("verbose msg=hi", "logfmt", "json").unwrap();
        assert_eq!(out, r#"[{"verbose":true,"msg":"hi"}]"#);
    }

    #[test]
    fn detect_types_off_keeps_strings() {
        let out = convert("n=42 ok=true", "logfmt", "json", "comma", false, false, true, "").unwrap();
        assert_eq!(out, r#"[{"n":"42","ok":"true"}]"#);
    }

    #[test]
    fn leading_zeros_stay_strings() {
        let out = run("zip=07030 phone=+15551234", "logfmt", "json").unwrap();
        assert_eq!(out, r#"[{"zip":"07030","phone":"+15551234"}]"#);
    }

    #[test]
    fn ndjson_to_csv_with_header_union() {
        let out = run(
            "{\"a\":1,\"b\":2}\n{\"a\":3,\"c\":4}",
            "ndjson",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b,c\n1,2,\n3,,4");
    }

    #[test]
    fn csv_to_logfmt_quotes_spaces() {
        let out = run("level,msg\ninfo,hello world", "csv", "logfmt").unwrap();
        assert_eq!(out, r#"level=info msg="hello world""#);
    }

    #[test]
    fn csv_delimiter_semicolon_both_ways() {
        let out = convert(
            "a;b\n1;2",
            "csv",
            "csv",
            "semicolon",
            true,
            false,
            true,
            "",
        )
        .unwrap();
        assert_eq!(out, "a;b\n1;2");
    }

    #[test]
    fn nested_json_flattens_to_logfmt() {
        let out = run(
            r#"[{"user":{"id":7,"name":"ada"},"tags":["x","y"]}]"#,
            "json",
            "logfmt",
        )
        .unwrap();
        assert_eq!(out, "user.id=7 user.name=ada tags.0=x tags.1=y");
    }

    #[test]
    fn flatten_off_writes_compact_json_values() {
        let out = convert(
            r#"[{"user":{"id":7}}]"#,
            "json",
            "logfmt",
            "comma",
            true,
            false,
            false,
            "",
        )
        .unwrap();
        assert_eq!(out, r#"user="{\"id\":7}""#);
    }

    #[test]
    fn keys_selects_and_orders_fields() {
        let out = convert(
            "a=1 b=2 c=3",
            "logfmt",
            "logfmt",
            "comma",
            true,
            false,
            true,
            "c, a",
        )
        .unwrap();
        assert_eq!(out, "c=3 a=1");
    }

    #[test]
    fn pretty_json_is_indented() {
        let out = convert("a=1", "logfmt", "json", "comma", true, true, true, "").unwrap();
        assert_eq!(out, "[\n  {\n    \"a\": 1\n  }\n]");
    }

    #[test]
    fn auto_detects_each_format() {
        assert_eq!(detect("a=1 b=2", ','), Some(Fmt::Logfmt));
        assert_eq!(detect("[{\"a\":1}]", ','), Some(Fmt::Json));
        assert_eq!(detect("{\"a\":1}\n{\"a\":2}", ','), Some(Fmt::Ndjson));
        assert_eq!(detect("{\n \"a\": 1\n}", ','), Some(Fmt::Json));
        assert_eq!(detect("a,b\n1,2", ','), Some(Fmt::Csv));
        assert_eq!(detect("just some prose", ','), None);
    }

    #[test]
    fn escapes_survive_logfmt_round_trip() {
        let out = run("[{\"msg\":\"line1\\nline2\\ttab\"}]", "json", "logfmt").unwrap();
        assert_eq!(out, r#"msg="line1\nline2\ttab""#);
        let back = run(&out, "logfmt", "ndjson").unwrap();
        assert_eq!(back, r#"{"msg":"line1\nline2\ttab"}"#);
    }

    #[test]
    fn logfmt_key_is_sanitized() {
        let out = run(r#"[{"bad key":"v","a=b":"w"}]"#, "json", "logfmt").unwrap();
        assert_eq!(out, "bad_key=v a_b=w");
    }

    #[test]
    fn empty_input_errors() {
        let err = run("   ", "auto", "json").unwrap_err();
        assert!(err.contains("no input"), "{err}");
    }

    #[test]
    fn undetectable_input_errors() {
        let err = run("this is just prose", "auto", "json").unwrap_err();
        assert!(err.contains("could not auto-detect"), "{err}");
    }

    #[test]
    fn bad_format_errors() {
        let err = run("a=1", "yaml", "json").unwrap_err();
        assert!(err.contains("unknown from format 'yaml'"), "{err}");
        let err = run("a=1", "auto", "yaml").unwrap_err();
        assert!(err.contains("unknown to format 'yaml'"), "{err}");
    }

    #[test]
    fn bad_delimiter_errors() {
        let err = convert("a,b\n1,2", "csv", "json", "colon", true, false, true, "").unwrap_err();
        assert!(err.contains("unknown delimiter 'colon'"), "{err}");
    }

    #[test]
    fn json_scalar_array_errors() {
        let err = run("[1,2,3]", "json", "logfmt").unwrap_err();
        assert!(err.contains("array item 1 is a number"), "{err}");
    }

    #[test]
    fn csv_header_only_errors() {
        let err = run("a,b", "csv", "json").unwrap_err();
        assert!(err.contains("only a header row"), "{err}");
    }

    #[test]
    fn invalid_ndjson_names_the_line() {
        let err = run("{\"a\":1}\nnot json", "ndjson", "json").unwrap_err();
        assert!(err.contains("invalid JSON on line 2"), "{err}");
    }

    #[test]
    fn input_cap_boundary() {
        let unit = "k=1\n"; // 4 chars
        let at = unit.repeat(MAX_INPUT_CHARS / 4);
        assert_eq!(at.chars().count(), MAX_INPUT_CHARS);
        assert!(run(&at, "logfmt", "ndjson").is_ok());
        let over = format!("{at}x");
        let err = run(&over, "logfmt", "ndjson").unwrap_err();
        assert!(err.contains("1000001 characters"), "{err}");
    }
}
