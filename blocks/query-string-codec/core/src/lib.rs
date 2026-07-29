//! query-string-codec core — a bidirectional URL query-string codec, shared by
//! the chat skill block and the web page. Pure Rust, no wafer/wasm-bindgen deps.
//!
//! Two directions:
//!  - **parse** (query string → JSON): splits on `&`/`;`, percent/`+`-decodes
//!    keys and values, groups repeated keys into arrays, and resolves bracket
//!    notation (`a[]`, `a[0]`, `a[key]`) into nested arrays/objects. The parse
//!    half mirrors the sibling `parse-query-string` block (same repo, MIT) but
//!    emits ONLY the structured JSON object so it round-trips through `build`.
//!  - **build** (JSON object → query string): the reverse — walks a JSON object
//!    and serializes it back to an encoded query string, with a choice of array
//!    serialization style (`brackets`/`indices`/`repeat`/`comma`), `+` vs `%20`
//!    for spaces, optional alphabetical key sorting, and an optional leading `?`.
//!    No existing block does this direction; it is the reason this tool exists.

use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Shared percent-encoding helpers
// ---------------------------------------------------------------------------

/// Decode a single hex nibble.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode a string. Invalid `%` escapes are left literal (lenient). When
/// `plus_as_space`, a `+` decodes to a space (form-urlencoded semantics). Bytes
/// are decoded UTF-8 lossily so non-UTF-8 sequences degrade to U+FFFD.
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

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Percent-encode `s` as a query-string component. Everything outside the RFC
/// 3986 "unreserved" set (`A-Z a-z 0-9 - _ . ~`) is escaped. When `space_as_plus`
/// a space becomes `+` (form semantics; a literal `+` still escapes to `%2B`);
/// otherwise a space becomes `%20`. Applied to key segment names and to values —
/// the `[` `]` that delimit bracket notation are added as literal structure by
/// the builder, never passed through here.
fn encode_component(s: &str, space_as_plus: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' if space_as_plus => out.push('+'),
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// PARSE direction (query string → structured JSON)
// ---------------------------------------------------------------------------

/// Strip a single leading `?` and any leading `&` from a bare query string.
fn normalize(query: &str) -> &str {
    let q = query.trim();
    let q = q.strip_prefix('?').unwrap_or(q);
    q.trim_start_matches('&')
}

/// Split a raw key into its base name and the chain of bracket segments.
/// `a[b][]` → ("a", ["b", ""]). A `[` with no matching `]` stays literal.
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
        return (raw.to_string(), Vec::new());
    }
    (base, segments)
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

/// Descend a bracket path, creating nested objects/arrays as needed and placing
/// `value` at the leaf. `""` = append-to-array; a numeric segment is an index;
/// anything else is an object key.
fn set_path(node: &mut Value, segments: &[String], value: Value) {
    if segments.is_empty() {
        *node = value;
        return;
    }
    let seg = &segments[0];
    let is_last = segments.len() == 1;

    if seg.is_empty() {
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

/// Parse a query string into a structured JSON object (as a `serde_json::Value`).
fn parse_to_value(query: &str, plus_as_space: bool) -> Value {
    let q = normalize(query);
    let mut structured = Map::new();
    if !q.is_empty() {
        for part in q.split(['&', ';']).filter(|p| !p.is_empty()) {
            let (raw_key, raw_val, has_eq) = match part.split_once('=') {
                Some((k, v)) => (k, v, true),
                None => (part, "", false),
            };
            let value = if has_eq {
                percent_decode(raw_val, plus_as_space)
            } else {
                String::new()
            };
            let (base_raw, seg_raw) = split_brackets(raw_key);
            let base = percent_decode(&base_raw, plus_as_space);
            let segments: Vec<String> = seg_raw
                .iter()
                .map(|s| percent_decode(s, plus_as_space))
                .collect();
            insert_structured(&mut structured, &base, &segments, Value::String(value));
        }
    }
    Value::Object(structured)
}

/// Parse a query string → pretty structured JSON. Errors only on empty input.
pub fn parse(query: &str, plus_as_space: bool) -> Result<String, String> {
    if query.trim().is_empty() {
        return Err("input is empty — paste a query string like `a=1&b=2&user[age]=30`".into());
    }
    let value = parse_to_value(query, plus_as_space);
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// BUILD direction (JSON object → query string)
// ---------------------------------------------------------------------------

/// How to serialize a JSON array back into query-string pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayStyle {
    /// `tags[]=a&tags[]=b` — empty brackets, appends (round-trips via parse).
    Brackets,
    /// `tags[0]=a&tags[1]=b` — explicit numeric indices (round-trips via parse).
    Indices,
    /// `tags=a&tags=b` — repeat the bare key (round-trips via parse).
    Repeat,
    /// `tags=a,b` — one pair, comma-joined scalars (compact; NOT auto-split on parse).
    Comma,
}

impl ArrayStyle {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "brackets" => Ok(ArrayStyle::Brackets),
            "indices" => Ok(ArrayStyle::Indices),
            "repeat" => Ok(ArrayStyle::Repeat),
            "comma" => Ok(ArrayStyle::Comma),
            other => Err(format!(
                "invalid array_style {other:?}: expected \"brackets\", \"indices\", \"repeat\", or \"comma\""
            )),
        }
    }
}

/// Render a JSON scalar as the raw (pre-encoding) string that goes on the wire.
/// `null` becomes an empty value. Returns `None` for a non-scalar (object/array).
fn scalar_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some(String::new()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Recursively serialize `value` under the already-encoded `key` path (bracket
/// delimiters are literal; segment names inside were encoded by the caller).
fn walk(
    key: &str,
    value: &Value,
    style: ArrayStyle,
    space_as_plus: bool,
    sort_keys: bool,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            if sort_keys {
                keys.sort();
            }
            for k in keys {
                let seg = encode_component(k, space_as_plus);
                let child = if key.is_empty() {
                    seg
                } else {
                    format!("{key}[{seg}]")
                };
                walk(&child, &map[k], style, space_as_plus, sort_keys, out)?;
            }
            Ok(())
        }
        Value::Array(arr) => match style {
            ArrayStyle::Comma => {
                let mut parts = Vec::with_capacity(arr.len());
                for item in arr {
                    let s = scalar_string(item).ok_or_else(|| {
                        format!(
                            "comma array style needs scalar values, but key {key:?} holds a nested object/array — use brackets, indices, or repeat"
                        )
                    })?;
                    parts.push(encode_component(&s, space_as_plus));
                }
                out.push((key.to_string(), parts.join(",")));
                Ok(())
            }
            ArrayStyle::Repeat => {
                for item in arr {
                    walk(key, item, style, space_as_plus, sort_keys, out)?;
                }
                Ok(())
            }
            ArrayStyle::Brackets => {
                let child = format!("{key}[]");
                for item in arr {
                    walk(&child, item, style, space_as_plus, sort_keys, out)?;
                }
                Ok(())
            }
            ArrayStyle::Indices => {
                for (i, item) in arr.iter().enumerate() {
                    let child = format!("{key}[{i}]");
                    walk(&child, item, style, space_as_plus, sort_keys, out)?;
                }
                Ok(())
            }
        },
        scalar => {
            // key is "" only for a top-level scalar, which build() rejects first.
            let s = scalar_string(scalar).unwrap_or_default();
            out.push((key.to_string(), encode_component(&s, space_as_plus)));
            Ok(())
        }
    }
}

/// Build a query string from a JSON object.
///
/// - `array_style`: how arrays serialize (see [`ArrayStyle`]).
/// - `space_as_plus`: `true` → space becomes `+` (form); `false` → `%20`.
/// - `sort_keys`: sort object keys alphabetically (recursively) instead of
///   keeping the input's key order.
/// - `prefix_question_mark`: prepend a leading `?` to the result.
pub fn build(
    input: &str,
    array_style: &str,
    space_as_plus: bool,
    sort_keys: bool,
    prefix_question_mark: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err(
            "input is empty — paste a JSON object like `{\"name\":\"John Doe\",\"tags\":[\"a\",\"b\"]}`"
                .into(),
        );
    }
    let style = ArrayStyle::parse(array_style)?;
    let value: Value =
        serde_json::from_str(input.trim()).map_err(|e| format!("invalid JSON: {e}"))?;
    if !value.is_object() {
        return Err(format!(
            "top-level JSON must be an object (like `{{\"key\":\"value\"}}`), got a {}",
            json_kind(&value)
        ));
    }
    let mut pairs: Vec<(String, String)> = Vec::new();
    walk("", &value, style, space_as_plus, sort_keys, &mut pairs)?;
    let body = pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    if prefix_question_mark {
        Ok(format!("?{body}"))
    } else {
        Ok(body)
    }
}

fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ---------------------------------------------------------------------------
// Dispatcher (shared by chat block + web page)
// ---------------------------------------------------------------------------

/// Run the codec. `direction` is `"parse"` (default) or `"build"`. Build-only
/// options are ignored in parse mode; `space_as_plus` applies to both (decode
/// `+`→space on parse; encode space→`+` on build).
#[allow(clippy::too_many_arguments)]
pub fn run(
    direction: &str,
    input: &str,
    array_style: &str,
    space_as_plus: bool,
    sort_keys: bool,
    prefix_question_mark: bool,
) -> Result<String, String> {
    match direction {
        "" | "parse" => parse(input, space_as_plus),
        "build" => build(input, array_style, space_as_plus, sort_keys, prefix_question_mark),
        other => Err(format!(
            "invalid direction {other:?}: expected \"parse\" or \"build\""
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse direction ----

    #[test]
    fn parse_simple_pairs() {
        let out = parse("a=1&b=2", true).unwrap();
        assert_eq!(out, "{\n  \"a\": \"1\",\n  \"b\": \"2\"\n}");
    }

    #[test]
    fn parse_strips_leading_question_mark_and_repeats() {
        let v: Value = serde_json::from_str(&parse("?color=red&color=blue", true).unwrap()).unwrap();
        assert_eq!(v, serde_json::json!({"color": ["red", "blue"]}));
    }

    #[test]
    fn parse_brackets_and_nested() {
        let v: Value =
            serde_json::from_str(&parse("tag[]=a&tag[]=b&user[age]=30", true).unwrap()).unwrap();
        assert_eq!(v, serde_json::json!({"tag": ["a", "b"], "user": {"age": "30"}}));
    }

    #[test]
    fn parse_plus_and_percent_decode() {
        let v: Value = serde_json::from_str(&parse("q=S%C3%A3o+Paulo", true).unwrap()).unwrap();
        assert_eq!(v, serde_json::json!({"q": "São Paulo"}));
        // plus kept literal when disabled
        let v2: Value = serde_json::from_str(&parse("q=a+b", false).unwrap()).unwrap();
        assert_eq!(v2, serde_json::json!({"q": "a+b"}));
    }

    #[test]
    fn parse_empty_is_error() {
        assert!(parse("   ", true).is_err());
        assert!(parse("", true).is_err());
    }

    // ---- build direction ----

    #[test]
    fn build_simple_pairs() {
        assert_eq!(
            build("{\"a\":\"1\",\"b\":\"2\"}", "brackets", true, false, false).unwrap(),
            "a=1&b=2"
        );
    }

    #[test]
    fn build_array_styles() {
        let j = "{\"tags\":[\"x\",\"y\"]}";
        assert_eq!(build(j, "brackets", true, false, false).unwrap(), "tags[]=x&tags[]=y");
        assert_eq!(build(j, "indices", true, false, false).unwrap(), "tags[0]=x&tags[1]=y");
        assert_eq!(build(j, "repeat", true, false, false).unwrap(), "tags=x&tags=y");
        assert_eq!(build(j, "comma", true, false, false).unwrap(), "tags=x,y");
    }

    #[test]
    fn build_nested_object() {
        assert_eq!(
            build("{\"user\":{\"name\":\"Ann\",\"age\":30}}", "brackets", true, false, false)
                .unwrap(),
            "user[name]=Ann&user[age]=30"
        );
    }

    #[test]
    fn build_space_encoding_and_specials() {
        assert_eq!(build("{\"q\":\"a b\"}", "brackets", true, false, false).unwrap(), "q=a+b");
        assert_eq!(build("{\"q\":\"a b\"}", "brackets", false, false, false).unwrap(), "q=a%20b");
        // a literal '+' escapes to %2B even in plus mode; reserved chars escape.
        assert_eq!(
            build("{\"q\":\"a+b&c\"}", "brackets", true, false, false).unwrap(),
            "q=a%2Bb%26c"
        );
    }

    #[test]
    fn build_sort_keys_and_prefix() {
        assert_eq!(build("{\"b\":\"2\",\"a\":\"1\"}", "brackets", true, false, false).unwrap(), "b=2&a=1");
        assert_eq!(build("{\"b\":\"2\",\"a\":\"1\"}", "brackets", true, true, false).unwrap(), "a=1&b=2");
        assert_eq!(build("{\"a\":\"1\"}", "brackets", true, false, true).unwrap(), "?a=1");
    }

    #[test]
    fn build_null_and_number_and_bool() {
        assert_eq!(
            build("{\"k\":null,\"n\":5,\"ok\":true}", "brackets", true, false, false).unwrap(),
            "k=&n=5&ok=true"
        );
    }

    #[test]
    fn build_array_of_objects_brackets() {
        assert_eq!(
            build("{\"items\":[{\"id\":1},{\"id\":2}]}", "brackets", true, false, false).unwrap(),
            "items[][id]=1&items[][id]=2"
        );
    }

    #[test]
    fn build_invalid_json_is_error() {
        let e = build("not json", "brackets", true, false, false).unwrap_err();
        assert!(e.contains("invalid JSON"), "got: {e}");
    }

    #[test]
    fn build_non_object_top_level_is_error() {
        let e = build("[1,2,3]", "brackets", true, false, false).unwrap_err();
        assert!(e.contains("top-level JSON must be an object"), "got: {e}");
    }

    #[test]
    fn build_comma_rejects_nested() {
        let e = build("{\"a\":[{\"x\":1}]}", "comma", true, false, false).unwrap_err();
        assert!(e.contains("comma array style"), "got: {e}");
    }

    #[test]
    fn build_empty_is_error() {
        assert!(build("   ", "brackets", true, false, false).is_err());
    }

    #[test]
    fn build_rejects_bad_array_style() {
        let e = build("{\"a\":\"1\"}", "json", true, false, false).unwrap_err();
        assert!(e.contains("invalid array_style"), "got: {e}");
    }

    // ---- round-trip via the dispatcher ----

    #[test]
    fn run_round_trips_build_then_parse() {
        let qs = run(
            "build",
            "{\"name\":\"John Doe\",\"tags\":[\"a\",\"b\"],\"user\":{\"age\":30}}",
            "brackets",
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(qs, "name=John+Doe&tags[]=a&tags[]=b&user[age]=30");
        let back: Value =
            serde_json::from_str(&run("parse", &qs, "brackets", true, false, false).unwrap())
                .unwrap();
        assert_eq!(
            back,
            serde_json::json!({"name": "John Doe", "tags": ["a", "b"], "user": {"age": "30"}})
        );
    }

    #[test]
    fn run_rejects_bad_direction() {
        let e = run("encode", "a=1", "brackets", true, false, false).unwrap_err();
        assert!(e.contains("invalid direction"), "got: {e}");
    }
}
