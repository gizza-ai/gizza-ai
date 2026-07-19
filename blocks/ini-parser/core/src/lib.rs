//! gizza-ai/ini-parser core — pure, no wafer/wasm-bindgen deps.
//!
//! Parses INI / `.conf` text (sections, key=value / key:value pairs, `;`/`#`
//! comments) into structured JSON and reports duplicate keys. Section-less
//! ("global") keys before the first `[section]` are supported. Duplicate keys
//! within a scope are handled per an explicit policy (last / first / array /
//! error), and every duplicate is reported in the `report` output.
//!
//! The chat schema is single-sourced from the block's `descriptor()`; this crate
//! is the pure engine shared by the chat block, the CLI, and the web page.

use serde_json::{Map, Number, Value};

/// Hard cap on input lines — a guard against pathological O(n²) duplicate scans
/// on a huge paste. A real config file is far under this.
pub const MAX_LINES: usize = 100_000;

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Nested config object: global keys at the root, each section a nested object.
    Json,
    /// Flat map of dotted keys (`section.key`, or a bare `key` for globals).
    Flat,
    /// `{ globals, sections, duplicates, stats }` — surfaces the duplicate report.
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" | "nested" => Output::Json,
            "flat" => Output::Flat,
            "report" => Output::Report,
            other => {
                return Err(format!("unknown output '{other}' (use json, flat, or report)"))
            }
        })
    }
}

/// What to do when a key appears more than once in the same scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DupPolicy {
    /// Keep the last value (at its first position). This is what most parsers do.
    Last,
    /// Keep the first value; ignore later ones.
    First,
    /// Collect every value for a duplicated key into a JSON array (in order).
    Array,
    /// Stop with an error listing the first duplicate.
    Error,
}

impl DupPolicy {
    pub fn parse(s: &str) -> Result<DupPolicy, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "last" => DupPolicy::Last,
            "first" => DupPolicy::First,
            "array" => DupPolicy::Array,
            "error" => DupPolicy::Error,
            other => {
                return Err(format!(
                    "unknown duplicate_keys '{other}' (use last, first, array, or error)"
                ))
            }
        })
    }
}

/// Which characters start a whole-line (or inline) comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comments {
    Both,
    Semicolon,
    Hash,
}

impl Comments {
    pub fn parse(s: &str) -> Result<Comments, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" => Comments::Both,
            "semicolon" | ";" => Comments::Semicolon,
            "hash" | "#" => Comments::Hash,
            other => {
                return Err(format!("unknown comments '{other}' (use both, semicolon, or hash)"))
            }
        })
    }

    fn prefixes(self) -> &'static [char] {
        match self {
            Comments::Both => &[';', '#'],
            Comments::Semicolon => &[';'],
            Comments::Hash => &['#'],
        }
    }
}

/// A parsed scope: the global (section-less) block, or a named section. Keys are
/// stored in encounter order; duplicate section headers merge into one scope.
struct Scope {
    /// `None` = the global (section-less) scope.
    name: Option<String>,
    /// (key, value) pairs in encounter order — duplicates kept for reporting.
    entries: Vec<(String, Value)>,
}

/// Parse INI/conf `text` and render it as JSON per the given options.
///
/// - `output`: `json` (nested config) | `flat` (dotted keys) | `report`.
/// - `duplicate_keys`: `last` | `first` | `array` | `error`.
/// - `detect_types`: when true, unquoted `true/false/yes/no/on/off` → boolean and
///   numeric-looking values → number; otherwise every value stays a string.
/// - `comments`: `both` (`;` and `#`) | `semicolon` | `hash`.
/// - `inline_comments`: when true, strip a trailing ` ;`/` #` comment from an
///   unquoted value (the comment char must be preceded by whitespace).
pub fn parse(
    text: &str,
    output: &str,
    duplicate_keys: &str,
    detect_types: bool,
    comments: &str,
    inline_comments: bool,
) -> Result<String, String> {
    let output = Output::parse(output)?;
    let policy = DupPolicy::parse(duplicate_keys)?;
    let comments = Comments::parse(comments)?;
    let prefixes = comments.prefixes();

    let mut scopes: Vec<Scope> = vec![Scope {
        name: None,
        entries: Vec::new(),
    }];
    let mut active = 0usize; // index into `scopes`; 0 = the global scope
    let mut comment_count = 0usize;

    for (i, raw_line) in text.lines().enumerate() {
        if i >= MAX_LINES {
            return Err(format!("input exceeds the {MAX_LINES}-line limit"));
        }
        let line_no = i + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        // Whole-line comment.
        if starts_with_any(line, prefixes) {
            comment_count += 1;
            continue;
        }
        // Section header: [name].
        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err(format!(
                    "line {line_no}: unterminated section header (expected '[name]'): {line}"
                ));
            }
            let name = line[1..line.len() - 1].trim().to_string();
            if name.is_empty() {
                return Err(format!("line {line_no}: empty section name '[]'"));
            }
            // Merge into an existing scope of the same name, else start a new one.
            active = match scopes.iter().position(|s| s.name.as_deref() == Some(name.as_str())) {
                Some(idx) => idx,
                None => {
                    scopes.push(Scope {
                        name: Some(name),
                        entries: Vec::new(),
                    });
                    scopes.len() - 1
                }
            };
            continue;
        }
        // key = value  /  key : value  (whichever of '=' or ':' comes first wins).
        let Some(delim) = line.char_indices().find(|&(_, c)| c == '=' || c == ':').map(|(i, _)| i)
        else {
            return Err(format!(
                "line {line_no}: expected 'key = value', '[section]', or a comment: {line}"
            ));
        };
        let key = line[..delim].trim();
        if key.is_empty() {
            return Err(format!(
                "line {line_no}: empty key before '{}'",
                &line[delim..delim + 1]
            ));
        }
        let raw_value = line[delim + 1..].trim();
        let value = parse_value(raw_value, detect_types, inline_comments, prefixes);
        scopes[active].entries.push((key.to_string(), value));
    }

    render(&scopes, output, policy, comment_count)
}

fn starts_with_any(s: &str, prefixes: &[char]) -> bool {
    s.chars().next().map(|c| prefixes.contains(&c)).unwrap_or(false)
}

/// Turn a raw value string into a JSON value: strip a surrounding matched quote
/// (quoted values are always strings), else optionally strip an inline comment
/// and optionally detect a boolean/number.
fn parse_value(raw: &str, detect_types: bool, inline_comments: bool, prefixes: &[char]) -> Value {
    // Quoted value → strip the matching outer quotes; keep the inside verbatim as
    // a string (never type-detected). Anything after the closing quote is dropped.
    if let Some(q) = raw.chars().next().filter(|c| *c == '"' || *c == '\'') {
        if let Some(end) = raw[1..].find(q) {
            return Value::String(raw[1..1 + end].to_string());
        }
    }
    let mut val = raw;
    if inline_comments {
        // A comment marker only counts when preceded by whitespace (configparser
        // semantics), so a value like `a#b` keeps the `#`.
        if let Some(cut) = inline_comment_start(val, prefixes) {
            val = val[..cut].trim_end();
        }
    }
    if detect_types {
        detect(val)
    } else {
        Value::String(val.to_string())
    }
}

/// Byte index where a whitespace-preceded comment marker begins, if any.
fn inline_comment_start(s: &str, prefixes: &[char]) -> Option<usize> {
    let mut prev_ws = false;
    for (i, c) in s.char_indices() {
        if prev_ws && prefixes.contains(&c) {
            return Some(i);
        }
        prev_ws = c.is_ascii_whitespace();
    }
    None
}

/// Configparser-style scalar detection for an unquoted value.
fn detect(s: &str) -> Value {
    if s.is_empty() {
        return Value::String(String::new());
    }
    match s.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => return Value::Bool(true),
        "false" | "no" | "off" => return Value::Bool(false),
        _ => {}
    }
    // Integer: optional sign then digits only.
    if is_integer(s) {
        if let Ok(n) = s.parse::<i64>() {
            return Value::Number(n.into());
        }
    }
    // Float: must contain a '.' and parse to a finite number.
    if s.contains('.') {
        if let Ok(f) = s.parse::<f64>() {
            if f.is_finite() {
                if let Some(n) = Number::from_f64(f) {
                    return Value::Number(n);
                }
            }
        }
    }
    Value::String(s.to_string())
}

fn is_integer(s: &str) -> bool {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// A reported duplicate key.
struct Dup {
    scope: String,
    key: String,
    count: usize,
    values: Vec<Value>,
}

/// Collapse one scope's ordered entries into a JSON object per `policy`, and
/// collect any duplicates. Returns an error only for `DupPolicy::Error`.
fn resolve_scope(scope: &Scope, policy: DupPolicy) -> Result<(Map<String, Value>, Vec<Dup>), String> {
    // Group values by key, preserving first-seen order.
    let mut grouped: Vec<(String, Vec<Value>)> = Vec::new();
    for (k, v) in &scope.entries {
        if let Some(slot) = grouped.iter_mut().find(|(kk, _)| kk == k) {
            slot.1.push(v.clone());
        } else {
            grouped.push((k.clone(), vec![v.clone()]));
        }
    }
    let scope_label = scope.name.clone().unwrap_or_else(|| "(global)".to_string());
    let mut dups = Vec::new();
    let mut obj = Map::new();
    for (k, vals) in &grouped {
        if vals.len() > 1 {
            if policy == DupPolicy::Error {
                return Err(format!(
                    "duplicate key '{k}' in {scope_label} ({} occurrences); set duplicate_keys to last, first, or array to allow",
                    vals.len()
                ));
            }
            dups.push(Dup {
                scope: scope_label.clone(),
                key: k.clone(),
                count: vals.len(),
                values: vals.clone(),
            });
        }
        let resolved = match policy {
            DupPolicy::Last | DupPolicy::Error => vals.last().cloned().unwrap(),
            DupPolicy::First => vals.first().cloned().unwrap(),
            DupPolicy::Array => {
                if vals.len() > 1 {
                    Value::Array(vals.clone())
                } else {
                    vals[0].clone()
                }
            }
        };
        obj.insert(k.clone(), resolved);
    }
    Ok((obj, dups))
}

fn render(
    scopes: &[Scope],
    output: Output,
    policy: DupPolicy,
    comment_count: usize,
) -> Result<String, String> {
    let mut globals = Map::new();
    let mut sections: Map<String, Value> = Map::new();
    let mut all_dups: Vec<Dup> = Vec::new();
    let mut key_total = 0usize;

    for scope in scopes {
        let (obj, dups) = resolve_scope(scope, policy)?;
        key_total += obj.len();
        all_dups.extend(dups);
        match &scope.name {
            None => globals = obj,
            Some(name) => {
                sections.insert(name.clone(), Value::Object(obj));
            }
        }
    }

    let out = match output {
        Output::Json => {
            // Globals at the root, then each section as a nested object.
            let mut root = globals;
            for (name, obj) in sections {
                root.insert(name, obj);
            }
            Value::Object(root)
        }
        Output::Flat => {
            let mut root = Map::new();
            for (k, v) in globals {
                root.insert(k, v);
            }
            for (name, obj) in &sections {
                if let Value::Object(o) = obj {
                    for (k, v) in o {
                        root.insert(format!("{name}.{k}"), v.clone());
                    }
                }
            }
            Value::Object(root)
        }
        Output::Report => {
            let dup_list: Vec<Value> = all_dups
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "scope": d.scope,
                        "key": d.key,
                        "count": d.count,
                        "values": d.values,
                    })
                })
                .collect();
            serde_json::json!({
                "globals": Value::Object(globals),
                "sections": Value::Object(sections),
                "duplicates": dup_list,
                "stats": {
                    "sections": scopes.iter().filter(|s| s.name.is_some()).count(),
                    "keys": key_total,
                    "comments": comment_count,
                    "duplicates": all_dups.len(),
                },
            })
        }
    };

    serde_json::to_string_pretty(&out).map_err(|e| format!("failed to serialize JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> Value {
        serde_json::from_str(&parse(text, "json", "last", false, "both", false).unwrap()).unwrap()
    }
    fn err(text: &str) -> String {
        parse(text, "json", "last", false, "both", false).unwrap_err()
    }

    #[test]
    fn happy_sections_and_globals() {
        let v = ok("app = demo\n\n[server]\nhost = localhost\nport = 8080\n; a comment\n[db]\nname = main\n");
        assert_eq!(v["app"], Value::String("demo".into()));
        assert_eq!(v["server"]["host"], Value::String("localhost".into()));
        assert_eq!(v["server"]["port"], Value::String("8080".into()));
        assert_eq!(v["db"]["name"], Value::String("main".into()));
    }

    #[test]
    fn colon_delimiter_and_quotes_and_inline() {
        let out = parse("[s]\ngreeting : \"hello world\"\npath = /tmp ; a note\n", "json", "last", false, "both", true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["s"]["greeting"], Value::String("hello world".into()));
        assert_eq!(v["s"]["path"], Value::String("/tmp".into()));
    }

    #[test]
    fn type_detection() {
        let out = parse("[x]\nflag = true\nn = 42\npi = 3.14\ntext = hello\nzip = \"007\"\n", "json", "last", true, "both", false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["x"]["flag"], Value::Bool(true));
        assert_eq!(v["x"]["n"], serde_json::json!(42));
        assert_eq!(v["x"]["pi"], serde_json::json!(3.14));
        assert_eq!(v["x"]["text"], Value::String("hello".into()));
        // A quoted numeric-looking value stays a string.
        assert_eq!(v["x"]["zip"], Value::String("007".into()));
    }

    #[test]
    fn duplicate_last_first_array() {
        let ini = "[s]\nk = 1\nk = 2\nk = 3\n";
        assert_eq!(ok(ini)["s"]["k"], Value::String("3".into()));
        let first: Value = serde_json::from_str(&parse(ini, "json", "first", false, "both", false).unwrap()).unwrap();
        assert_eq!(first["s"]["k"], Value::String("1".into()));
        let arr: Value = serde_json::from_str(&parse(ini, "json", "array", false, "both", false).unwrap()).unwrap();
        assert_eq!(arr["s"]["k"], serde_json::json!(["1", "2", "3"]));
    }

    #[test]
    fn duplicate_report_and_stats() {
        let out = parse("g = 1\n[s]\nk = a\nk = b\n; note\n", "report", "last", false, "both", false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["globals"]["g"], Value::String("1".into()));
        assert_eq!(v["sections"]["s"]["k"], Value::String("b".into()));
        assert_eq!(v["duplicates"][0]["key"], Value::String("k".into()));
        assert_eq!(v["duplicates"][0]["count"], serde_json::json!(2));
        assert_eq!(v["duplicates"][0]["scope"], Value::String("s".into()));
        assert_eq!(v["stats"]["sections"], serde_json::json!(1));
        assert_eq!(v["stats"]["comments"], serde_json::json!(1));
        assert_eq!(v["stats"]["duplicates"], serde_json::json!(1));
    }

    #[test]
    fn flat_output_and_merged_sections() {
        // The section [s] appears twice — its keys merge into one scope.
        let out = parse("[s]\na = 1\n[t]\nb = 2\n[s]\nc = 3\n", "flat", "last", false, "both", false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["s.a"], Value::String("1".into()));
        assert_eq!(v["s.c"], Value::String("3".into()));
        assert_eq!(v["t.b"], Value::String("2".into()));
    }

    #[test]
    fn hash_only_comments_keep_semicolons() {
        // With comments=hash, a ';' line is NOT a comment → it's an error line.
        let e = parse("[s]\n; not a comment here\n", "json", "last", false, "hash", false).unwrap_err();
        assert!(e.contains("expected 'key = value'"), "got: {e}");
    }

    #[test]
    fn error_on_bad_line() {
        let e = err("[s]\nthis line has no delimiter\n");
        assert!(e.contains("line 2") && e.contains("expected"), "got: {e}");
    }

    #[test]
    fn error_on_duplicate_policy() {
        let e = parse("[s]\nk = 1\nk = 2\n", "json", "error", false, "both", false).unwrap_err();
        assert!(e.contains("duplicate key 'k'"), "got: {e}");
    }

    #[test]
    fn error_on_unterminated_section() {
        let e = err("[oops\n");
        assert!(e.contains("unterminated section header"), "got: {e}");
    }
}
