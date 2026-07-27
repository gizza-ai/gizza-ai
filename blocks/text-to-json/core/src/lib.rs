//! text-to-json core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Parses pasted structured text — INI, key=value config, logfmt, CSV, or
//! `/etc/passwd`-style colon-delimited text — into JSON. The format is
//! auto-detected by default (and reported in `output = "report"`), or pinned
//! with the `format` argument. Values are coerced to booleans/numbers by default
//! (`detect_types`); turn that off to keep every value a lossless string.

use serde_json::{Map, Value};

/// One of the five text shapes this tool understands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fmt {
    Ini,
    KeyValue,
    Logfmt,
    Csv,
    Passwd,
}

impl Fmt {
    fn name(self) -> &'static str {
        match self {
            Fmt::Ini => "ini",
            Fmt::KeyValue => "keyvalue",
            Fmt::Logfmt => "logfmt",
            Fmt::Csv => "csv",
            Fmt::Passwd => "passwd",
        }
    }
}

/// Convert `text` to JSON.
///
/// - `format`: `auto` (or empty) auto-detects; else pin to `ini` | `keyvalue` |
///   `logfmt` | `csv` | `passwd`.
/// - `detect_types`: coerce unquoted `true`/`false` to booleans and numeric
///   values to numbers (empty → null); false keeps every value a string.
/// - `pretty`: pretty-print with 2-space indentation (else minified). Ignored
///   for `output = "ndjson"`.
/// - `output`: `json` (the parsed value), `ndjson` (one record per line), or
///   `report` (`{ detected_format, record_count, data }`).
pub fn convert(
    text: &str,
    format: &str,
    detect_types: bool,
    pretty: bool,
    output: &str,
) -> Result<String, String> {
    let output = match output.trim().to_ascii_lowercase().as_str() {
        "" | "json" => Out::Json,
        "ndjson" => Out::Ndjson,
        "report" => Out::Report,
        other => {
            return Err(format!(
                "unknown output '{other}': expected json, ndjson, or report"
            ))
        }
    };

    let pinned = parse_format(format)?;
    if text.trim().is_empty() {
        return Err("no input: paste some INI, key=value, logfmt, CSV, or passwd text to parse".into());
    }

    let fmt = pinned.unwrap_or_else(|| detect(text));
    let value = match fmt {
        Fmt::Ini => parse_ini(text, detect_types),
        Fmt::KeyValue => parse_keyvalue(text, detect_types),
        Fmt::Logfmt => parse_logfmt(text, detect_types),
        Fmt::Csv => parse_csv(text, detect_types)?,
        Fmt::Passwd => parse_passwd(text, detect_types),
    };

    let count = match &value {
        Value::Array(a) => a.len(),
        Value::Object(m) => m.len(),
        _ => 0,
    };

    match output {
        Out::Ndjson => Ok(to_ndjson(&value)),
        Out::Json => Ok(serialize(&value, pretty)),
        Out::Report => {
            let mut wrap = Map::new();
            wrap.insert("detected_format".into(), Value::String(fmt.name().into()));
            wrap.insert("record_count".into(), Value::Number(count.into()));
            wrap.insert("data".into(), value);
            Ok(serialize(&Value::Object(wrap), pretty))
        }
    }
}

enum Out {
    Json,
    Ndjson,
    Report,
}

fn parse_format(s: &str) -> Result<Option<Fmt>, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(None),
        "ini" | "conf" => Ok(Some(Fmt::Ini)),
        "keyvalue" | "key-value" | "kv" | "env" | "properties" => Ok(Some(Fmt::KeyValue)),
        "logfmt" => Ok(Some(Fmt::Logfmt)),
        "csv" | "tsv" => Ok(Some(Fmt::Csv)),
        "passwd" => Ok(Some(Fmt::Passwd)),
        other => Err(format!(
            "unknown format '{other}': expected auto, ini, keyvalue, logfmt, csv, or passwd"
        )),
    }
}

fn serialize(v: &Value, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(v).unwrap_or_default()
    } else {
        serde_json::to_string(v).unwrap_or_default()
    }
}

/// One compact JSON record per line. An object/scalar is a single line; an array
/// emits one line per element (the classic NDJSON / JSON-Lines shape).
fn to_ndjson(v: &Value) -> String {
    match v {
        Value::Array(a) => a
            .iter()
            .map(|e| serde_json::to_string(e).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n"),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// ---- detection -----------------------------------------------------------

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('#') || t.starts_with(';')
}

fn is_section(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && t.starts_with('[') && t.ends_with(']')
}

/// Count space-separated `key=…` pairs in a line (used only for detection).
fn logfmt_pair_count(line: &str) -> usize {
    line.split_whitespace()
        .filter(|tok| {
            let key = tok.split('=').next().unwrap_or("");
            tok.contains('=')
                && !key.is_empty()
                && key.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-'))
        })
        .count()
}

fn detect(text: &str) -> Fmt {
    let data: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !is_comment(l))
        .collect();
    if data.is_empty() {
        return Fmt::KeyValue;
    }

    // INI: any [section] header.
    if data.iter().any(|l| is_section(l)) {
        return Fmt::Ini;
    }

    // logfmt: at least one line packs two or more space-separated key=value pairs.
    if data.iter().any(|l| logfmt_pair_count(l) >= 2) {
        return Fmt::Logfmt;
    }

    // passwd: colon-delimited, no '=', consistent field count of 4+.
    if looks_passwd(&data) {
        return Fmt::Passwd;
    }

    // keyvalue: most lines carry a '=' or ':' separator (one pair per line).
    let kv = data
        .iter()
        .filter(|l| l.contains('=') || l.contains(':'))
        .count();
    if kv * 10 >= data.len() * 7 {
        return Fmt::KeyValue;
    }

    // CSV: a delimiter splits the rows into a consistent 2+ columns.
    if sniff_delimiter(&data).is_some() {
        return Fmt::Csv;
    }

    Fmt::KeyValue
}

fn looks_passwd(data: &[&str]) -> bool {
    if data.iter().any(|l| l.contains('=')) {
        return false;
    }
    let first = data[0].split(':').count();
    if first < 4 {
        return false;
    }
    let consistent = data
        .iter()
        .filter(|l| l.split(':').count() == first)
        .count();
    consistent * 10 >= data.len() * 8
}

/// Pick the delimiter (`,` `;` tab `|`) that most consistently splits the rows
/// into the same 2-or-more column count.
fn sniff_delimiter(data: &[&str]) -> Option<char> {
    let mut best: Option<(char, usize, usize)> = None; // (delim, agreeing_rows, cols)
    for &d in &[',', ';', '\t', '|'] {
        let sample: Vec<usize> = data
            .iter()
            .take(20)
            .map(|l| split_delimited(l, d).len())
            .collect();
        // The modal column count among the sampled rows.
        let cols = sample.iter().copied().max().unwrap_or(0);
        if cols < 2 {
            continue;
        }
        for c in 2..=cols {
            let agree = sample.iter().filter(|&&n| n == c).count();
            let better = match best {
                None => true,
                Some((_, ba, bc)) => agree > ba || (agree == ba && c > bc),
            };
            if better && agree * 2 >= sample.len() {
                best = Some((d, agree, c));
            }
        }
    }
    best.map(|(d, _, _)| d)
}

// ---- shared value helpers ------------------------------------------------

/// Strip one layer of matching single or double quotes; report whether the
/// value was quoted (quoted values are always kept as strings).
fn strip_quotes(s: &str) -> (&str, bool) {
    let b = s.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        (&s[1..s.len() - 1], true)
    } else {
        (s, false)
    }
}

/// Coerce a raw string value to a JSON value. Quoted values stay strings; with
/// `detect` off every value stays a string. With `detect` on, `true`/`false`
/// become booleans, integers/decimals become numbers, and an empty value → null.
fn coerce(raw: &str, quoted: bool, detect: bool) -> Value {
    if quoted || !detect {
        return Value::String(raw.to_string());
    }
    let t = raw.trim();
    if t.is_empty() {
        return Value::Null;
    }
    match t.to_ascii_lowercase().as_str() {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = t.parse::<i64>() {
        return Value::Number(i.into());
    }
    if let Ok(f) = t.parse::<f64>() {
        if f.is_finite() {
            if let Some(n) = serde_json::Number::from_f64(f) {
                return Value::Number(n);
            }
        }
    }
    Value::String(raw.to_string())
}

/// Split a `key = value` / `key: value` line on the FIRST `=` or `:`.
fn split_pair(line: &str) -> Option<(&str, &str)> {
    let eq = line.find('=');
    let colon = line.find(':');
    let idx = match (eq, colon) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    Some((&line[..idx], &line[idx + 1..]))
}

fn insert_pair(map: &mut Map<String, Value>, key: &str, val: &str, detect: bool) {
    let key = key.trim();
    if key.is_empty() {
        return;
    }
    let (v, quoted) = strip_quotes(val.trim());
    map.insert(key.to_string(), coerce(v, quoted, detect));
}

// ---- per-format parsers --------------------------------------------------

fn parse_ini(text: &str, detect: bool) -> Value {
    let mut root = Map::new();
    let mut sections: Map<String, Value> = Map::new();
    let mut current: Option<String> = None;

    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || is_comment(t) {
            continue;
        }
        if is_section(t) {
            let name = t[1..t.len() - 1].trim().to_string();
            sections.entry(name.clone()).or_insert_with(|| Value::Object(Map::new()));
            current = Some(name);
            continue;
        }
        if let Some((k, v)) = split_pair(t) {
            match &current {
                Some(sec) => {
                    if let Some(Value::Object(m)) = sections.get_mut(sec) {
                        insert_pair(m, k, v, detect);
                    }
                }
                None => insert_pair(&mut root, k, v, detect),
            }
        }
    }
    for (k, v) in sections {
        root.insert(k, v);
    }
    Value::Object(root)
}

fn parse_keyvalue(text: &str, detect: bool) -> Value {
    let mut map = Map::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || is_comment(t) {
            continue;
        }
        // Java-style `.properties` also allow `key value` (whitespace), but the
        // dominant shapes are `key=value` / `key: value`.
        let t = t.strip_prefix("export ").unwrap_or(t); // shell/env exports
        if let Some((k, v)) = split_pair(t) {
            insert_pair(&mut map, k, v, detect);
        }
    }
    Value::Object(map)
}

fn parse_logfmt(text: &str, detect: bool) -> Value {
    let mut rows = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || is_comment(t) {
            continue;
        }
        let mut obj = Map::new();
        for (k, v, quoted) in logfmt_pairs(t) {
            if k.is_empty() {
                continue;
            }
            match v {
                Some(val) => {
                    obj.insert(k, coerce(&val, quoted, detect));
                }
                // A bare key (no '='): logfmt treats it as a boolean flag.
                None => {
                    obj.insert(k, if detect { Value::Bool(true) } else { Value::String(String::new()) });
                }
            }
        }
        rows.push(Value::Object(obj));
    }
    Value::Array(rows)
}

/// Tokenize a logfmt line into `(key, Option<value>, quoted)` triples, honoring
/// double-quoted values that may contain spaces and escaped quotes.
fn logfmt_pairs(line: &str) -> Vec<(String, Option<String>, bool)> {
    let mut out = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let n = chars.len();
    while i < n {
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= n {
            break;
        }
        // read key up to '=' or whitespace
        let start = i;
        while i < n && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if i < n && chars[i] == '=' {
            i += 1; // consume '='
            if i < n && chars[i] == '"' {
                i += 1;
                let mut val = String::new();
                while i < n {
                    if chars[i] == '\\' && i + 1 < n {
                        val.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    if chars[i] == '"' {
                        i += 1;
                        break;
                    }
                    val.push(chars[i]);
                    i += 1;
                }
                out.push((key, Some(val), true));
            } else {
                let vstart = i;
                while i < n && !chars[i].is_whitespace() {
                    i += 1;
                }
                let val: String = chars[vstart..i].iter().collect();
                out.push((key, Some(val), false));
            }
        } else {
            out.push((key, None, false));
        }
    }
    out
}

fn parse_csv(text: &str, detect: bool) -> Result<Value, String> {
    let data: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if data.len() < 2 {
        return Err(
            "csv needs a header row and at least one data row (or pick a different format)".into(),
        );
    }
    let delim = sniff_delimiter(&data).unwrap_or(',');
    let header: Vec<String> = split_delimited(data[0], delim)
        .into_iter()
        .map(|(f, _)| f.trim().to_string())
        .collect();

    let mut rows = Vec::new();
    for line in &data[1..] {
        let fields = split_delimited(line, delim);
        let mut obj = Map::new();
        for (i, (val, quoted)) in fields.iter().enumerate() {
            let key = header
                .get(i)
                .filter(|h| !h.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("field{}", i + 1));
            obj.insert(key, coerce(val, *quoted, detect));
        }
        rows.push(Value::Object(obj));
    }
    Ok(Value::Array(rows))
}

/// Split one line on `delim`, honoring RFC-4180 double-quoting (`""` → literal
/// `"`). Returns `(field, was_quoted)` per column.
fn split_delimited(line: &str, delim: char) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut started_quoted = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let n = chars.len();
    let mut field_has_content = false;
    while i < n {
        let c = chars[i];
        if in_quotes {
            if c == '"' {
                if i + 1 < n && chars[i + 1] == '"' {
                    field.push('"');
                    i += 2;
                    continue;
                }
                in_quotes = false;
                i += 1;
                continue;
            }
            field.push(c);
            i += 1;
        } else if c == '"' && !field_has_content {
            in_quotes = true;
            quoted = true;
            started_quoted = true;
            field_has_content = true;
            i += 1;
        } else if c == delim {
            out.push((std::mem::take(&mut field), quoted));
            quoted = false;
            started_quoted = false;
            field_has_content = false;
            i += 1;
        } else {
            field.push(c);
            field_has_content = true;
            i += 1;
        }
    }
    let _ = started_quoted;
    out.push((field, quoted));
    out
}

const PASSWD_FIELDS: [&str; 7] = ["name", "password", "uid", "gid", "gecos", "home", "shell"];
const GROUP_FIELDS: [&str; 4] = ["name", "password", "gid", "members"];

fn parse_passwd(text: &str, detect: bool) -> Value {
    let mut rows = Vec::new();
    for line in text.lines() {
        let t = line.trim_end();
        if t.trim().is_empty() || is_comment(t) {
            continue;
        }
        let fields: Vec<&str> = t.split(':').collect();
        let names: &[&str] = match fields.len() {
            7 => &PASSWD_FIELDS,
            4 => &GROUP_FIELDS,
            _ => &[],
        };
        let mut obj = Map::new();
        for (i, f) in fields.iter().enumerate() {
            let key = names
                .get(i)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("field{}", i + 1));
            obj.insert(key, coerce(f, false, detect));
        }
        rows.push(Value::Object(obj));
    }
    Value::Array(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(v: &str) -> Value {
        serde_json::from_str(v).unwrap()
    }

    #[test]
    fn ini_nested_with_globals() {
        let out = convert("app = demo\n\n[server]\nhost = localhost\nport = 8080", "auto", true, false, "json").unwrap();
        assert_eq!(
            j(&out),
            j(r#"{"app":"demo","server":{"host":"localhost","port":8080}}"#)
        );
    }

    #[test]
    fn keyvalue_flat_object_no_types() {
        let out = convert("HOST=localhost\nPORT=8080\n# comment\nDEBUG=true", "keyvalue", false, false, "json").unwrap();
        assert_eq!(
            j(&out),
            j(r#"{"HOST":"localhost","PORT":"8080","DEBUG":"true"}"#)
        );
    }

    #[test]
    fn keyvalue_export_prefix_and_types() {
        let out = convert("export PORT=8080\nDEBUG=false", "keyvalue", true, false, "json").unwrap();
        assert_eq!(j(&out), j(r#"{"PORT":8080,"DEBUG":false}"#));
    }

    #[test]
    fn logfmt_array_with_quotes_and_bare_key() {
        let out = convert(
            "level=info msg=\"started up\" port=8080 tls\nlevel=error msg=\"it broke\"",
            "auto",
            true,
            false,
            "json",
        )
        .unwrap();
        assert_eq!(
            j(&out),
            j(r#"[{"level":"info","msg":"started up","port":8080,"tls":true},{"level":"error","msg":"it broke"}]"#)
        );
    }

    #[test]
    fn csv_array_of_objects_type_infer() {
        let out = convert("name,age,admin\nAlice,30,true\nBob,25,false", "auto", true, false, "json").unwrap();
        assert_eq!(
            j(&out),
            j(r#"[{"name":"Alice","age":30,"admin":true},{"name":"Bob","age":25,"admin":false}]"#)
        );
    }

    #[test]
    fn csv_semicolon_and_quoted_field() {
        let out = convert("city;note\nParis;\"a, b\"\nLyon;plain", "csv", true, false, "json").unwrap();
        assert_eq!(
            j(&out),
            j(r#"[{"city":"Paris","note":"a, b"},{"city":"Lyon","note":"plain"}]"#)
        );
    }

    #[test]
    fn passwd_named_fields_uid_gid_numeric() {
        let out = convert(
            "root:x:0:0:root:/root:/bin/bash\ndaemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin",
            "auto",
            true,
            false,
            "json",
        )
        .unwrap();
        assert_eq!(
            j(&out),
            j(r#"[{"name":"root","password":"x","uid":0,"gid":0,"gecos":"root","home":"/root","shell":"/bin/bash"},{"name":"daemon","password":"x","uid":1,"gid":1,"gecos":"daemon","home":"/usr/sbin","shell":"/usr/sbin/nologin"}]"#)
        );
    }

    #[test]
    fn report_surfaces_detected_format() {
        let out = convert("a=1 b=2\nc=3 d=4", "auto", true, false, "report").unwrap();
        let v = j(&out);
        assert_eq!(v["detected_format"], j("\"logfmt\""));
        assert_eq!(v["record_count"], j("2"));
        assert_eq!(v["data"], j(r#"[{"a":1,"b":2},{"c":3,"d":4}]"#));
    }

    #[test]
    fn ndjson_one_record_per_line() {
        let out = convert("name,x\nA,1\nB,2", "csv", true, false, "ndjson").unwrap();
        assert_eq!(out, "{\"name\":\"A\",\"x\":1}\n{\"name\":\"B\",\"x\":2}");
    }

    #[test]
    fn pretty_output_has_newlines() {
        let out = convert("k=v", "keyvalue", false, true, "json").unwrap();
        assert!(out.contains('\n') && out.contains("  \"k\""));
    }

    #[test]
    fn quoted_value_stays_string_even_with_detect() {
        let out = convert("port = \"8080\"", "ini", true, false, "json").unwrap();
        assert_eq!(j(&out), j(r#"{"port":"8080"}"#));
    }

    #[test]
    fn error_on_empty_input() {
        let err = convert("   \n  ", "auto", true, false, "json").unwrap_err();
        assert!(err.contains("no input"), "got: {err}");
    }

    #[test]
    fn error_on_unknown_format() {
        let err = convert("k=v", "yaml", true, false, "json").unwrap_err();
        assert!(err.contains("unknown format"), "got: {err}");
    }

    #[test]
    fn error_on_unknown_output() {
        let err = convert("k=v", "auto", true, false, "xml").unwrap_err();
        assert!(err.contains("unknown output"), "got: {err}");
    }

    #[test]
    fn error_on_csv_single_row() {
        let err = convert("just,one,row", "csv", true, false, "json").unwrap_err();
        assert!(err.contains("header row"), "got: {err}");
    }

    #[test]
    fn detect_types_off_keeps_numbers_as_strings() {
        let out = convert("name,age\nAlice,30", "csv", false, false, "json").unwrap();
        assert_eq!(j(&out), j(r#"[{"name":"Alice","age":"30"}]"#));
    }

    #[test]
    fn empty_csv_field_becomes_null_with_detect() {
        let out = convert("a,b\n1,", "csv", true, false, "json").unwrap();
        assert_eq!(j(&out), j(r#"[{"a":1,"b":null}]"#));
    }
}
