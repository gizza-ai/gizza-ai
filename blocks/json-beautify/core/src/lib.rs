//! gizza-ai/json-beautify core — pretty-print (or minify) JSON with configurable
//! indentation, validating it in the process. Key order is preserved
//! (serde_json `preserve_order`). Pure-Rust.

use serde::Serialize;
use serde_json::Value;

/// Format `json`. `indent` spaces per level (clamped 0..=8); `indent == 0`
/// produces compact single-line output (minify). Returns a validation error with
/// line/column if the input isn't valid JSON.
pub fn format(json: &str, indent: usize) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON input".into());
    }
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let n = indent.min(8);

    if n == 0 {
        return serde_json::to_string(&value).map_err(|e| format!("serialize failed: {e}"));
    }
    let pad = vec![b' '; n];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value.serialize(&mut ser).map_err(|e| format!("serialize failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_default() {
        let out = format(r#"{"b":1,"a":[1,2]}"#, 2).unwrap();
        assert_eq!(out, "{\n  \"b\": 1,\n  \"a\": [\n    1,\n    2\n  ]\n}");
    }

    #[test]
    fn preserves_key_order() {
        // b before a in the input → b before a in the output (not sorted).
        let out = format(r#"{"b":1,"a":2}"#, 2).unwrap();
        assert!(out.find("\"b\"").unwrap() < out.find("\"a\"").unwrap());
    }

    #[test]
    fn indent_four() {
        let out = format(r#"{"a":1}"#, 4).unwrap();
        assert_eq!(out, "{\n    \"a\": 1\n}");
    }

    #[test]
    fn minify_with_zero() {
        let out = format("{\n  \"a\" : 1,\n  \"b\":[1, 2]\n}", 0).unwrap();
        assert_eq!(out, r#"{"a":1,"b":[1,2]}"#);
    }

    #[test]
    fn validates() {
        assert!(format("{bad}", 2).is_err());
        assert!(format("", 2).is_err());
        assert!(format("[1,2,]", 2).is_err()); // trailing comma is invalid JSON
    }

    #[test]
    fn scalars_and_nesting() {
        assert_eq!(format("42", 2).unwrap(), "42");
        let out = format(r#"{"x":{"y":[true,null,"z"]}}"#, 2).unwrap();
        assert!(out.contains("\"y\": [") && out.contains("null"));
    }
}
