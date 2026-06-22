//! gizza-ai/parse-query-string core — parse a URL query string into structured
//! data. Splits on `&` (and `;`), percent-decodes keys and values (with `+`
//! decoding to a space in form mode), preserves the ordered raw pairs, and
//! builds a structured object that groups repeated keys and resolves bracket
//! notation (`a[]=1&a[]=2` → array; `a[k]=v` → nested object; `a[0]=x` →
//! indexed array). Pure-Rust, no wafer/wasm-bindgen deps. Shared by the chat
//! block and the page.

use serde::Serialize;
use serde_json::{Map, Value};

/// A single decoded `key=value` pair, in source order. `value` is `None` for a
/// bare key with no `=` (e.g. `?flag`); an empty value (`?k=`) is `Some("")`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pair {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// The result of parsing a query string.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedQuery {
    /// The number of `key[=value]` pairs found.
    pub count: usize,
    /// Every pair in source order, with duplicates preserved.
    pub pairs: Vec<Pair>,
    /// A structured object: repeated keys become arrays, and bracket notation
    /// (`a[]`, `a[0]`, `a[key]`) is expanded into nested arrays/objects.
    pub structured: Value,
}

/// Decode a hex nibble.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode a string. Invalid `%` escapes are left literal (lenient — a
/// stray `%` shouldn't error). When `plus_as_space`, a `+` decodes to a space
/// (`application/x-www-form-urlencoded` semantics). Bytes are decoded as UTF-8
/// lossily so non-UTF-8 sequences degrade to U+FFFD.
fn percent_decode(s: &str, plus_as_space: bool) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        if plus_as_space && b == b'+' {
            out.push(b' ');
        } else {
            out.push(b);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Strip a single leading `?` and any leading `&` from a bare query string.
fn normalize(query: &str) -> &str {
    let q = query.trim();
    let q = q.strip_prefix('?').unwrap_or(q);
    q.trim_start_matches('&')
}

/// Split a raw key into its base name and the chain of bracket segments.
/// `a[b][]` → ("a", ["b", ""]). `a` → ("a", []). A `[` with no matching `]`
/// is treated as a literal part of the name.
fn split_brackets(raw: &str) -> (String, Vec<String>) {
    let open = match raw.find('[') {
        Some(i) => i,
        None => return (raw.to_string(), Vec::new()),
    };
    let base = raw[..open].to_string();
    let mut segments = Vec::new();
    let mut rest = &raw[open..];
    while rest.starts_with('[') {
        match rest.find(']') {
            Some(close) => {
                segments.push(rest[1..close].to_string());
                rest = &rest[close + 1..];
            }
            None => break,
        }
    }
    if segments.is_empty() {
        // `[` had no closing `]` — treat the whole raw key literally.
        return (raw.to_string(), Vec::new());
    }
    (base, segments)
}

/// Insert a decoded value into the structured tree following the bracket path.
fn insert_structured(root: &mut Map<String, Value>, base: &str, segments: &[String], value: Value) {
    if segments.is_empty() {
        merge_scalar(root, base, value);
        return;
    }
    let entry = root
        .entry(base.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    set_path(entry, segments, value);
}

/// If `key` already exists, turn it into / append to an array; else set scalar.
fn merge_scalar(map: &mut Map<String, Value>, key: &str, value: Value) {
    match map.get_mut(key) {
        None => {
            map.insert(key.to_string(), value);
        }
        Some(existing) => {
            if let Value::Array(arr) = existing {
                arr.push(value);
            } else {
                let prev = existing.take();
                *existing = Value::Array(vec![prev, value]);
            }
        }
    }
}

/// Descend a bracket path inside an already-created container `node`, creating
/// nested objects/arrays as needed and placing `value` at the leaf. An empty
/// segment `""` means "append to array"; a numeric segment is an array index;
/// anything else is an object key.
fn set_path(node: &mut Value, segments: &[String], value: Value) {
    if segments.is_empty() {
        *node = value;
        return;
    }
    let seg = &segments[0];
    let is_last = segments.len() == 1;

    if seg.is_empty() {
        // Append-to-array notation `[]`.
        if !node.is_array() {
            *node = Value::Array(Vec::new());
        }
        let arr = node.as_array_mut().unwrap();
        if is_last {
            arr.push(value);
        } else {
            let mut child = Value::Object(Map::new());
            set_path(&mut child, &segments[1..], value);
            arr.push(child);
        }
        return;
    }

    if let Ok(idx) = seg.parse::<usize>() {
        // Numeric index → array, padded with nulls.
        if !node.is_array() {
            *node = Value::Array(Vec::new());
        }
        let arr = node.as_array_mut().unwrap();
        while arr.len() <= idx {
            arr.push(Value::Null);
        }
        if is_last {
            arr[idx] = value;
        } else {
            if !arr[idx].is_object() && !arr[idx].is_array() {
                arr[idx] = Value::Object(Map::new());
            }
            set_path(&mut arr[idx], &segments[1..], value);
        }
        return;
    }

    // Named key → object.
    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    let obj = node.as_object_mut().unwrap();
    if is_last {
        merge_scalar(obj, seg, value);
    } else {
        let child = obj
            .entry(seg.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        set_path(child, &segments[1..], value);
    }
}

/// Parse a query string. `plus_as_space` toggles `+`→space decoding (form
/// semantics; default on). Always succeeds — an empty/whitespace query yields
/// zero pairs rather than an error.
pub fn parse(query: &str, plus_as_space: bool) -> ParsedQuery {
    let q = normalize(query);
    let mut pairs: Vec<Pair> = Vec::new();
    let mut structured = Map::new();

    if !q.is_empty() {
        for part in q.split(['&', ';']).filter(|p| !p.is_empty()) {
            let (raw_key, raw_val, has_eq) = match part.split_once('=') {
                Some((k, v)) => (k, v, true),
                None => (part, "", false),
            };
            let key = percent_decode(raw_key, plus_as_space);
            let value = if has_eq {
                Some(percent_decode(raw_val, plus_as_space))
            } else {
                None
            };
            pairs.push(Pair {
                key: key.clone(),
                value: value.clone(),
            });

            // Structured view: decode bracket segments individually.
            let (base_raw, seg_raw) = split_brackets(raw_key);
            let base = percent_decode(&base_raw, plus_as_space);
            let segments: Vec<String> = seg_raw
                .iter()
                .map(|s| percent_decode(s, plus_as_space))
                .collect();
            let json_val = Value::String(value.unwrap_or_default());
            insert_structured(&mut structured, &base, &segments, json_val);
        }
    }

    ParsedQuery {
        count: pairs.len(),
        pairs,
        structured: Value::Object(structured),
    }
}

/// Parse and return pretty JSON (chat / programmatic surface).
pub fn run(query: &str, plus_as_space: bool) -> Result<String, String> {
    let parsed = parse(query, plus_as_space);
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(query: &str, plus_as_space: bool) -> Result<String, String> {
    let p = parse(query, plus_as_space);
    let mut out = String::new();
    out.push_str(&format!("Parameters ({}):\n", p.count));
    if p.pairs.is_empty() {
        out.push_str("  (none — empty query string)\n");
    } else {
        for pair in &p.pairs {
            match &pair.value {
                Some(v) if v.is_empty() => out.push_str(&format!("  {} = (empty)\n", pair.key)),
                Some(v) => out.push_str(&format!("  {} = {}\n", pair.key, v)),
                None => out.push_str(&format!("  {} (no value)\n", pair.key)),
            }
        }
    }
    out.push_str("\nStructured:\n");
    out.push_str(&serde_json::to_string_pretty(&p.structured).map_err(|e| e.to_string())?);
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_simple_pairs() {
        let p = parse("a=1&b=2&c=3", true);
        assert_eq!(p.count, 3);
        assert_eq!(p.pairs[0].key, "a");
        assert_eq!(p.pairs[0].value.as_deref(), Some("1"));
        assert_eq!(p.pairs[2].key, "c");
        assert_eq!(p.structured, json!({"a": "1", "b": "2", "c": "3"}));
    }

    #[test]
    fn strips_leading_question_mark() {
        let p = parse("?x=1&y=2", true);
        assert_eq!(p.count, 2);
        assert_eq!(p.structured, json!({"x": "1", "y": "2"}));
    }

    #[test]
    fn percent_decodes_keys_and_values() {
        let p = parse("na%20me=John%20Doe&city=S%C3%A3o%20Paulo", true);
        assert_eq!(p.pairs[0].key, "na me");
        assert_eq!(p.pairs[0].value.as_deref(), Some("John Doe"));
        assert_eq!(p.pairs[1].value.as_deref(), Some("São Paulo"));
    }

    #[test]
    fn plus_decodes_to_space_when_enabled() {
        let p = parse("q=a+b+c", true);
        assert_eq!(p.pairs[0].value.as_deref(), Some("a b c"));
    }

    #[test]
    fn plus_kept_literal_when_disabled() {
        let p = parse("q=a+b", false);
        assert_eq!(p.pairs[0].value.as_deref(), Some("a+b"));
    }

    #[test]
    fn repeated_keys_grouped_into_array() {
        let p = parse("color=red&color=green&color=blue", true);
        assert_eq!(p.count, 3);
        assert_eq!(p.structured, json!({"color": ["red", "green", "blue"]}));
    }

    #[test]
    fn empty_bracket_appends_to_array() {
        let p = parse("tag[]=a&tag[]=b", true);
        assert_eq!(p.structured, json!({"tag": ["a", "b"]}));
    }

    #[test]
    fn numeric_bracket_index() {
        let p = parse("x[0]=a&x[2]=c", true);
        assert_eq!(p.structured, json!({"x": ["a", null, "c"]}));
    }

    #[test]
    fn named_bracket_nested_object() {
        let p = parse("user[name]=Ann&user[age]=30", true);
        assert_eq!(p.structured, json!({"user": {"name": "Ann", "age": "30"}}));
    }

    #[test]
    fn deeply_nested_brackets() {
        let p = parse("a[b][c]=v", true);
        assert_eq!(p.structured, json!({"a": {"b": {"c": "v"}}}));
    }

    #[test]
    fn array_of_objects() {
        let p = parse("items[][id]=1&items[][id]=2", true);
        assert_eq!(p.structured, json!({"items": [{"id": "1"}, {"id": "2"}]}));
    }

    #[test]
    fn bare_key_no_value() {
        let p = parse("flag&k=v", true);
        assert_eq!(p.pairs[0].key, "flag");
        assert_eq!(p.pairs[0].value, None);
        assert_eq!(p.structured, json!({"flag": "", "k": "v"}));
    }

    #[test]
    fn empty_value() {
        let p = parse("k=", true);
        assert_eq!(p.pairs[0].value.as_deref(), Some(""));
        assert_eq!(p.structured, json!({"k": ""}));
    }

    #[test]
    fn semicolon_separator() {
        let p = parse("a=1;b=2", true);
        assert_eq!(p.count, 2);
        assert_eq!(p.pairs[1].key, "b");
    }

    #[test]
    fn empty_query_yields_no_pairs() {
        let p = parse("", true);
        assert_eq!(p.count, 0);
        assert!(p.pairs.is_empty());
        assert_eq!(p.structured, json!({}));
        let q = parse("   ?  ", true);
        assert_eq!(q.count, 0);
    }

    #[test]
    fn lenient_on_bad_percent_escape() {
        let p = parse("k=100%off", true);
        assert_eq!(p.pairs[0].value.as_deref(), Some("100%off"));
    }

    #[test]
    fn skips_empty_pairs() {
        let p = parse("a=1&&b=2&", true);
        assert_eq!(p.count, 2);
    }

    #[test]
    fn run_emits_valid_json() {
        let j = run("a=1&b[]=x&b[]=y", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["count"], 3);
        assert_eq!(v["structured"]["a"], "1");
        assert_eq!(v["structured"]["b"][1], "y");
    }

    #[test]
    fn render_is_human_readable() {
        let out = render("name=Ann&tags[]=a&tags[]=b", true).unwrap();
        assert!(out.contains("Parameters (3)"));
        assert!(out.contains("name = Ann"));
        assert!(out.contains("Structured:"));
    }
}
