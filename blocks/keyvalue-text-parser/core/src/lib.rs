//! gizza-ai/keyvalue-text-parser core — pure, no wafer/wasm-bindgen deps.
//!
//! Turns freeform `key: value` / `key = value` text into structured JSON.
//! Unlike a config-file parser it expects MESSY input: lines that carry no
//! separator (prose, headings, banners) are skipped by default, blank lines can
//! split the text into repeated RECORDS, and a key that appears more than once
//! is grouped into an array instead of being silently overwritten.
//!
//! The chat schema is single-sourced from the block's `descriptor()`; this crate
//! is the pure engine shared by the chat block, the CLI, and the web page.

use serde::Serialize;
use serde_json::{Map, Number, Value};

/// Hard cap on input lines — a guard against a pathological paste. A real
/// email-header dump, log block or form export is far under this.
pub const MAX_LINES: usize = 10_000;

/// Which character(s) split a line into key and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Separator {
    /// Whichever of `:` or `=` appears FIRST on the line (per line).
    Auto,
    Colon,
    Equals,
    Tab,
    Pipe,
    /// An explicit separator string supplied by the caller.
    Custom(String),
}

impl Separator {
    pub fn parse(s: &str, custom: &str) -> Result<Separator, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Separator::Auto,
            "colon" => Separator::Colon,
            "equals" => Separator::Equals,
            "tab" => Separator::Tab,
            "pipe" => Separator::Pipe,
            "custom" => {
                if custom.is_empty() {
                    return Err(
                        "separator='custom' needs a custom_separator (e.g. '->' or ' - ')".into(),
                    );
                }
                Separator::Custom(custom.to_string())
            }
            other => {
                return Err(format!(
                    "unknown separator '{other}' (use auto, colon, equals, tab, pipe, or custom)"
                ))
            }
        })
    }

    /// Byte offset + length of the first separator occurrence in `line`.
    fn find(&self, line: &str) -> Option<(usize, usize)> {
        match self {
            Separator::Auto => {
                let colon = line.find(':');
                let equals = line.find('=');
                match (colon, equals) {
                    (Some(c), Some(e)) => Some((c.min(e), 1)),
                    (Some(c), None) => Some((c, 1)),
                    (None, Some(e)) => Some((e, 1)),
                    (None, None) => None,
                }
            }
            Separator::Colon => line.find(':').map(|i| (i, 1)),
            Separator::Equals => line.find('=').map(|i| (i, 1)),
            Separator::Tab => line.find('\t').map(|i| (i, 1)),
            Separator::Pipe => line.find('|').map(|i| (i, 1)),
            Separator::Custom(s) => line.find(s.as_str()).map(|i| (i, s.len())),
        }
    }

    /// Human-readable form used in error messages.
    fn label(&self) -> String {
        match self {
            Separator::Auto => "':' or '='".into(),
            Separator::Colon => "':'".into(),
            Separator::Equals => "'='".into(),
            Separator::Tab => "a tab".into(),
            Separator::Pipe => "'|'".into(),
            Separator::Custom(s) => format!("'{s}'"),
        }
    }
}

/// The shape of the JSON that comes out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Structure {
    /// One flat JSON object for the whole input (blank lines are ignored).
    Object,
    /// An array of objects — one per blank-line-separated block of text.
    Records,
    /// An ordered array of `{ key, value, line }` entries, duplicates kept.
    Pairs,
}

impl Structure {
    pub fn parse(s: &str) -> Result<Structure, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "object" => Structure::Object,
            "records" => Structure::Records,
            "pairs" => Structure::Pairs,
            other => {
                return Err(format!(
                    "unknown structure '{other}' (use object, records, or pairs)"
                ))
            }
        })
    }
}

/// What to do when the same key shows up twice in one object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplicates {
    /// Collect every value for a repeated key into an array, in order.
    Group,
    /// Keep the last value (at the first occurrence's position).
    Last,
    /// Keep the first value; drop later ones.
    First,
    /// Fail on the first repeated key.
    Error,
}

impl Duplicates {
    pub fn parse(s: &str) -> Result<Duplicates, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "group" => Duplicates::Group,
            "last" => Duplicates::Last,
            "first" => Duplicates::First,
            "error" => Duplicates::Error,
            other => {
                return Err(format!(
                    "unknown duplicates '{other}' (use group, last, first, or error)"
                ))
            }
        })
    }
}

/// How keys are normalized before they land in the JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCase {
    AsIs,
    Lower,
    Snake,
}

impl KeyCase {
    pub fn parse(s: &str) -> Result<KeyCase, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "as-is" | "asis" => KeyCase::AsIs,
            "lower" => KeyCase::Lower,
            "snake" => KeyCase::Snake,
            other => {
                return Err(format!(
                    "unknown key_case '{other}' (use as-is, lower, or snake)"
                ))
            }
        })
    }

    fn apply(&self, key: &str) -> String {
        match self {
            KeyCase::AsIs => key.to_string(),
            KeyCase::Lower => key.to_lowercase(),
            KeyCase::Snake => {
                let mut out = String::with_capacity(key.len());
                for ch in key.chars() {
                    if ch.is_alphanumeric() {
                        out.extend(ch.to_lowercase());
                    } else if !out.ends_with('_') {
                        out.push('_');
                    }
                }
                out.trim_matches('_').to_string()
            }
        }
    }
}

/// What to do with a line that carries no separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmatched {
    /// Ignore it (the default — freeform text is full of prose lines).
    Skip,
    /// Fail, naming the line.
    Error,
}

impl Unmatched {
    pub fn parse(s: &str) -> Result<Unmatched, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "skip" => Unmatched::Skip,
            "error" => Unmatched::Error,
            other => return Err(format!("unknown unmatched '{other}' (use skip or error)")),
        })
    }
}

/// Every knob the parser takes, so callers don't juggle a dozen positionals.
#[derive(Debug, Clone)]
pub struct Options {
    pub separator: Separator,
    pub structure: Structure,
    pub duplicates: Duplicates,
    pub trim: bool,
    pub unquote: bool,
    /// Comma-separated list of line-comment markers, e.g. `#,;,//`. Empty = none.
    pub comment_prefixes: Vec<String>,
    pub infer_types: bool,
    pub key_case: KeyCase,
    pub unmatched: Unmatched,
    /// Spaces of indentation, 0 = minified single line. Capped at 8.
    pub indent: usize,
}

/// One parsed `key = value` entry, before it is folded into an object.
struct Pair {
    key: String,
    value: Value,
    line: usize,
}

fn split_prefixes(list: &str) -> Vec<String> {
    list.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Parse the string forms of every option (what the CLI, chat and page all send).
#[allow(clippy::too_many_arguments)]
pub fn options_from_strings(
    separator: &str,
    custom_separator: &str,
    structure: &str,
    duplicates: &str,
    trim: bool,
    unquote: bool,
    comment_prefixes: &str,
    infer_types: bool,
    key_case: &str,
    unmatched: &str,
    indent: f64,
) -> Result<Options, String> {
    if !(0.0..=8.0).contains(&indent) {
        return Err(format!(
            "indent must be between 0 and 8 spaces (got {indent})"
        ));
    }
    Ok(Options {
        separator: Separator::parse(separator, custom_separator.trim_matches('"'))?,
        structure: Structure::parse(structure)?,
        duplicates: Duplicates::parse(duplicates)?,
        trim,
        unquote,
        comment_prefixes: split_prefixes(comment_prefixes),
        infer_types,
        key_case: KeyCase::parse(key_case)?,
        unmatched: Unmatched::parse(unmatched)?,
        indent: indent as usize,
    })
}

/// Convenience wrapper used by the chat block, CLI and web page: takes the raw
/// string/bool option forms, returns the JSON document as a string.
#[allow(clippy::too_many_arguments)]
pub fn parse_text(
    text: &str,
    separator: &str,
    custom_separator: &str,
    structure: &str,
    duplicates: &str,
    trim: bool,
    unquote: bool,
    comment_prefixes: &str,
    infer_types: bool,
    key_case: &str,
    unmatched: &str,
    indent: f64,
) -> Result<String, String> {
    let opts = options_from_strings(
        separator,
        custom_separator,
        structure,
        duplicates,
        trim,
        unquote,
        comment_prefixes,
        infer_types,
        key_case,
        unmatched,
        indent,
    )?;
    let value = parse(text, &opts)?;
    render(&value, opts.indent)
}

/// Serialize a JSON value with `indent` spaces (0 = minified).
pub fn render(value: &Value, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize failed: {e}"));
    }
    let pad = " ".repeat(indent);
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

/// Parse `text` into a JSON value per `opts`.
pub fn parse(text: &str, opts: &Options) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Err("no input text — paste some 'key: value' or 'key = value' lines".into());
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "input has {} lines, over the {MAX_LINES}-line limit — split it and parse in parts",
            lines.len()
        ));
    }

    // Blocks of pairs: a new block starts after a blank line. `Object`/`Pairs`
    // flatten them again; `Records` keeps one object per block.
    let mut blocks: Vec<Vec<Pair>> = vec![Vec::new()];
    for (idx, raw) in lines.iter().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let no = idx + 1;
        if line.trim().is_empty() {
            if !blocks.last().map(|b| b.is_empty()).unwrap_or(true) {
                blocks.push(Vec::new());
            }
            continue;
        }
        let lead = line.trim_start();
        if opts.comment_prefixes.iter().any(|p| lead.starts_with(p)) {
            continue;
        }
        let Some((at, len)) = opts.separator.find(line) else {
            match opts.unmatched {
                Unmatched::Skip => continue,
                Unmatched::Error => {
                    return Err(format!(
                        "line {no} has no separator ({}): {}",
                        opts.separator.label(),
                        snippet(line)
                    ))
                }
            }
        };
        let (raw_key, raw_value) = (&line[..at], &line[at + len..]);
        let key_src = if opts.trim { raw_key.trim() } else { raw_key };
        if key_src.trim().is_empty() {
            match opts.unmatched {
                Unmatched::Skip => continue,
                Unmatched::Error => {
                    return Err(format!(
                        "line {no} has an empty key before the separator: {}",
                        snippet(line)
                    ))
                }
            }
        }
        let mut value_src = if opts.trim {
            raw_value.trim()
        } else {
            raw_value
        };
        let mut quoted = false;
        if opts.unquote {
            if let Some(inner) = strip_quotes(value_src) {
                value_src = inner;
                quoted = true;
            }
        }
        let value = if opts.infer_types && !quoted {
            infer(value_src)
        } else {
            Value::String(value_src.to_string())
        };
        blocks
            .last_mut()
            .expect("a block always exists")
            .push(Pair {
                key: opts.key_case.apply(key_src),
                value,
                line: no,
            });
    }

    let blocks: Vec<Vec<Pair>> = blocks.into_iter().filter(|b| !b.is_empty()).collect();
    if blocks.is_empty() {
        return Err(format!(
            "no {} pairs found — check the separator setting",
            opts.separator.label()
        ));
    }

    Ok(match opts.structure {
        Structure::Object => {
            let all: Vec<Pair> = blocks.into_iter().flatten().collect();
            Value::Object(fold(all, opts.duplicates)?)
        }
        Structure::Records => {
            let mut out = Vec::with_capacity(blocks.len());
            for block in blocks {
                out.push(Value::Object(fold(block, opts.duplicates)?));
            }
            Value::Array(out)
        }
        Structure::Pairs => {
            let mut out = Vec::new();
            for pair in blocks.into_iter().flatten() {
                let mut entry = Map::new();
                entry.insert("key".into(), Value::String(pair.key));
                entry.insert("value".into(), pair.value);
                entry.insert("line".into(), Value::Number(Number::from(pair.line as u64)));
                out.push(Value::Object(entry));
            }
            Value::Array(out)
        }
    })
}

/// Fold ordered pairs into one object, applying the duplicate policy.
fn fold(pairs: Vec<Pair>, policy: Duplicates) -> Result<Map<String, Value>, String> {
    let mut map: Map<String, Value> = Map::new();
    // Keys already turned into a group array, so a third value appends rather
    // than wrapping an array inside an array.
    let mut grouped: Vec<String> = Vec::new();
    for pair in pairs {
        if !map.contains_key(&pair.key) {
            map.insert(pair.key, pair.value);
            continue;
        }
        match policy {
            Duplicates::Error => {
                return Err(format!(
                    "duplicate key '{}' on line {} (use duplicates=group to collect repeats)",
                    pair.key, pair.line
                ))
            }
            Duplicates::First => {}
            Duplicates::Last => {
                map.insert(pair.key, pair.value);
            }
            Duplicates::Group => {
                // Update in place so the key keeps its first-seen position.
                let Some(slot) = map.get_mut(&pair.key) else {
                    continue;
                };
                if grouped.contains(&pair.key) {
                    if let Value::Array(items) = slot {
                        items.push(pair.value);
                    }
                } else {
                    let first = slot.take();
                    *slot = Value::Array(vec![first, pair.value]);
                    grouped.push(pair.key);
                }
            }
        }
    }
    Ok(map)
}

/// `"quoted"` / `'quoted'` → the inner text, if the quotes match and wrap it.
fn strip_quotes(v: &str) -> Option<&str> {
    let bytes = v.as_bytes();
    if bytes.len() >= 2 {
        let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
        if (first == b'"' || first == b'\'') && first == last {
            return Some(&v[1..v.len() - 1]);
        }
    }
    None
}

/// Coerce an unquoted value to a JSON boolean/null/number where it clearly is one.
fn infer(v: &str) -> Value {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => return Value::Bool(true),
        "false" | "no" | "off" => return Value::Bool(false),
        "null" | "nil" | "none" => return Value::Null,
        _ => {}
    }
    let t = v.trim();
    if t.is_empty() {
        return Value::String(v.to_string());
    }
    // Reject leading zeros / plus signs so IDs, zip codes and phone numbers stay
    // strings (0042 must not become 42).
    let digits = t.strip_prefix('-').unwrap_or(t);
    if digits.starts_with('+')
        || (digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0."))
    {
        return Value::String(v.to_string());
    }
    if let Ok(i) = t.parse::<i64>() {
        return Value::Number(Number::from(i));
    }
    if let Ok(f) = t.parse::<f64>() {
        if f.is_finite() {
            if let Some(n) = Number::from_f64(f) {
                return Value::Number(n);
            }
        }
    }
    Value::String(v.to_string())
}

/// Trim a line for use inside an error message.
fn snippet(line: &str) -> String {
    let t = line.trim();
    if t.chars().count() <= 60 {
        return format!("'{t}'");
    }
    let short: String = t.chars().take(57).collect();
    format!("'{short}...'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            separator: Separator::Auto,
            structure: Structure::Object,
            duplicates: Duplicates::Group,
            trim: true,
            unquote: true,
            comment_prefixes: split_prefixes("#,;,//"),
            infer_types: false,
            key_case: KeyCase::AsIs,
            unmatched: Unmatched::Skip,
            indent: 2,
        }
    }

    fn run(text: &str, o: &Options) -> String {
        render(&parse(text, o).unwrap(), o.indent).unwrap()
    }

    #[test]
    fn happy_path_colon_and_equals() {
        let out = run("Name: Ada\nrole = engineer", &opts());
        assert_eq!(out, "{\n  \"Name\": \"Ada\",\n  \"role\": \"engineer\"\n}");
    }

    #[test]
    fn error_on_no_pairs() {
        let err = parse("just some prose with no pairs", &opts()).unwrap_err();
        assert!(err.contains("no ':' or '=' pairs found"), "got: {err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = parse("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("no input text"), "got: {err}");
    }

    #[test]
    fn repeats_group_into_an_array() {
        let out = run("tag: a\ntag: b\ntag: c\nname: x", &opts());
        assert_eq!(
            out,
            "{\n  \"tag\": [\n    \"a\",\n    \"b\",\n    \"c\"\n  ],\n  \"name\": \"x\"\n}"
        );
    }

    #[test]
    fn duplicates_last_first_and_error() {
        let mut o = opts();
        o.indent = 0;
        o.duplicates = Duplicates::Last;
        assert_eq!(run("k: a\nk: b", &o), r#"{"k":"b"}"#);
        o.duplicates = Duplicates::First;
        assert_eq!(run("k: a\nk: b", &o), r#"{"k":"a"}"#);
        o.duplicates = Duplicates::Error;
        let err = parse("k: a\nk: b", &o).unwrap_err();
        assert!(err.contains("duplicate key 'k' on line 2"), "got: {err}");
    }

    #[test]
    fn records_split_on_blank_lines() {
        let mut o = opts();
        o.indent = 0;
        o.structure = Structure::Records;
        let out = run("name: a\nid: 1\n\nname: b\nid: 2\n", &o);
        assert_eq!(out, r#"[{"name":"a","id":"1"},{"name":"b","id":"2"}]"#);
    }

    #[test]
    fn pairs_keep_order_and_line_numbers() {
        let mut o = opts();
        o.indent = 0;
        o.structure = Structure::Pairs;
        let out = run("a: 1\n\nnot a pair\na: 2", &o);
        assert_eq!(
            out,
            r#"[{"key":"a","value":"1","line":1},{"key":"a","value":"2","line":4}]"#
        );
    }

    #[test]
    fn unmatched_error_names_the_line() {
        let mut o = opts();
        o.unmatched = Unmatched::Error;
        let err = parse("a: 1\nplain heading", &o).unwrap_err();
        assert!(
            err.contains("line 2 has no separator (':' or '=')"),
            "got: {err}"
        );
    }

    #[test]
    fn auto_takes_the_first_separator_on_the_line() {
        let mut o = opts();
        o.indent = 0;
        assert_eq!(
            run("url: https://x.dev/a=b", &o),
            r#"{"url":"https://x.dev/a=b"}"#
        );
        assert_eq!(run("expr = a:b", &o), r#"{"expr":"a:b"}"#);
    }

    #[test]
    fn explicit_separators_tab_pipe_custom() {
        let mut o = opts();
        o.indent = 0;
        o.separator = Separator::Tab;
        assert_eq!(run("host\tlocal:host", &o), r#"{"host":"local:host"}"#);
        o.separator = Separator::Pipe;
        assert_eq!(run("host|local", &o), r#"{"host":"local"}"#);
        o.separator = Separator::Custom("->".into());
        assert_eq!(run("host -> local", &o), r#"{"host":"local"}"#);
        o.separator = Separator::Colon;
        assert_eq!(run("a=1: x", &o), r#"{"a=1":"x"}"#);
        o.separator = Separator::Equals;
        assert_eq!(run("a:1 = x", &o), r#"{"a:1":"x"}"#);
    }

    #[test]
    fn custom_separator_requires_a_value() {
        let err = Separator::parse("custom", "").unwrap_err();
        assert!(err.contains("needs a custom_separator"), "got: {err}");
    }

    #[test]
    fn comments_are_skipped_and_configurable() {
        let mut o = opts();
        o.indent = 0;
        assert_eq!(run("# note: x\na: 1\n// b: 2\n; c: 3", &o), r#"{"a":"1"}"#);
        o.comment_prefixes = split_prefixes("");
        assert_eq!(run("# note: x\na: 1", &o), r##"{"# note":"x","a":"1"}"##);
    }

    #[test]
    fn type_inference_and_id_safety() {
        let mut o = opts();
        o.indent = 0;
        o.infer_types = true;
        let out = run(
            "n: 42\nf: 1.5\nb: yes\nz: null\nid: 0042\nphone: +15550100\nq: \"7\"",
            &o,
        );
        assert_eq!(
            out,
            r#"{"n":42,"f":1.5,"b":true,"z":null,"id":"0042","phone":"+15550100","q":"7"}"#
        );
    }

    #[test]
    fn unquote_and_trim_toggles() {
        let mut o = opts();
        o.indent = 0;
        assert_eq!(
            run("a: \"hello world\"\nb: 'x'", &o),
            r#"{"a":"hello world","b":"x"}"#
        );
        o.unquote = false;
        assert_eq!(run("a: \"hi\"", &o), r#"{"a":"\"hi\""}"#);
        o.trim = false;
        o.unquote = true;
        assert_eq!(run("  a :  b  ", &o), "{\"  a \":\"  b  \"}");
    }

    #[test]
    fn key_case_normalization() {
        let mut o = opts();
        o.indent = 0;
        o.key_case = KeyCase::Lower;
        assert_eq!(
            run("Content-Type: text/html", &o),
            r#"{"content-type":"text/html"}"#
        );
        o.key_case = KeyCase::Snake;
        assert_eq!(
            run("Content-Type: text/html", &o),
            r#"{"content_type":"text/html"}"#
        );
        assert_eq!(run(" First  Name : Ada", &o), r#"{"first_name":"Ada"}"#);
    }

    #[test]
    fn indent_controls_formatting() {
        let mut o = opts();
        o.indent = 4;
        assert_eq!(run("a: 1", &o), "{\n    \"a\": \"1\"\n}");
        o.indent = 0;
        assert_eq!(run("a: 1", &o), r#"{"a":"1"}"#);
    }

    #[test]
    fn line_cap_boundary() {
        let at_cap = (1..=MAX_LINES)
            .map(|i| format!("k{i}: {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse(&at_cap, &opts()).is_ok());
        let over = format!("{at_cap}\nk{}: x", MAX_LINES + 1);
        let err = parse(&over, &opts()).unwrap_err();
        assert!(
            err.contains(&format!("over the {MAX_LINES}-line limit")),
            "got: {err}"
        );
    }

    #[test]
    fn string_option_forms_are_validated() {
        assert!(parse_text(
            "a: 1", "auto", "", "object", "group", true, true, "#", false, "as-is", "skip", 2.0
        )
        .is_ok());
        let err = parse_text(
            "a: 1", "nope", "", "object", "group", true, true, "#", false, "as-is", "skip", 2.0,
        )
        .unwrap_err();
        assert!(err.contains("unknown separator 'nope'"), "got: {err}");
        let err = parse_text(
            "a: 1", "auto", "", "object", "group", true, true, "#", false, "as-is", "skip", 9.0,
        )
        .unwrap_err();
        assert!(err.contains("indent must be between 0 and 8"), "got: {err}");
    }

    #[test]
    fn crlf_input_is_handled() {
        let mut o = opts();
        o.indent = 0;
        assert_eq!(run("a: 1\r\nb: 2\r\n", &o), r#"{"a":"1","b":"2"}"#);
    }
}
