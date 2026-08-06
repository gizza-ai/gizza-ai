//! gizza-ai/json-remove-nulls core — recursively drop object keys whose value is
//! `null` (and, opt-in, whose value is an empty string / array / object) from a
//! JSON document, then re-serialize it pretty or minified.
//!
//! The input is parsed and validated first, so malformed JSON is rejected with
//! the parser's exact line/column instead of producing garbled output. Key order
//! is preserved (serde_json `preserve_order`), and every value that isn't opted
//! into removal survives byte-for-byte — notably `false` and `0`, which are NOT
//! empty. Pure-Rust; shared by the chat skill block and the web page.

use serde::Serialize;
use serde_json::Value;

/// What to do with values that qualify for removal when they sit directly
/// inside an array (rather than under an object key).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arrays {
    /// Drop them, closing the gap: `[1, null, 2]` → `[1, 2]`.
    Compact,
    /// Leave array elements exactly as they are, so positions/indices are
    /// stable: `[1, null, 2]` → `[1, null, 2]`. Objects nested inside the
    /// array are still pruned internally.
    Keep,
}

impl Arrays {
    /// Parse the `arrays` param. Blank/unknown falls back to `compact`.
    pub fn parse(s: &str) -> Arrays {
        match s.trim().to_ascii_lowercase().as_str() {
            "keep" | "preserve" | "false" | "no" => Arrays::Keep,
            _ => Arrays::Compact,
        }
    }
}

/// Options controlling what counts as removable and how the result is printed.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Also remove values that are the empty string `""`.
    pub remove_empty_strings: bool,
    /// Also remove values that are the empty array `[]`.
    pub remove_empty_arrays: bool,
    /// Also remove values that are the empty object `{}`.
    pub remove_empty_objects: bool,
    /// Trim leading/trailing whitespace from every string value first, so a
    /// whitespace-only string becomes `""` (and is then removable).
    pub trim_strings: bool,
    /// Whether removable values inside arrays are compacted away or kept.
    pub arrays: Arrays,
    /// Spaces of indentation per level (clamped 0..=8). `0` minifies to one
    /// compact line.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            remove_empty_strings: false,
            remove_empty_arrays: false,
            remove_empty_objects: false,
            trim_strings: false,
            arrays: Arrays::Compact,
            indent: 2,
        }
    }
}

/// Parse `json`, recursively remove null (and opted-in empty) values, and
/// re-serialize. Returns a validation error with line/column if the input isn't
/// valid JSON.
///
/// The ROOT value is never dropped: a document that prunes down to nothing comes
/// back as `{}` / `[]` / `null` rather than as an empty string, so the output is
/// always valid JSON.
pub fn remove_nulls(json: &str, opts: Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON input: paste a JSON document such as {\"a\": 1, \"b\": null}".into());
    }
    let mut value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    prune(&mut value, &opts);
    render(&value, opts.indent)
}

/// Depth-first, bottom-up prune. Children are cleaned BEFORE the parent decides
/// what to drop, so removal cascades: if `remove_empty_objects` is on, an object
/// emptied by the prune itself also disappears (and can empty its own parent).
fn prune(value: &mut Value, opts: &Options) {
    match value {
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                prune(v, opts);
            }
            map.retain(|_, v| !removable(v, opts));
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                prune(v, opts);
            }
            if opts.arrays == Arrays::Compact {
                arr.retain(|v| !removable(v, opts));
            }
        }
        Value::String(s) => {
            if opts.trim_strings {
                let trimmed = s.trim();
                if trimmed.len() != s.len() {
                    *s = trimmed.to_string();
                }
            }
        }
        // Numbers and booleans are never modified — `0` and `false` are values,
        // not emptiness.
        _ => {}
    }
}

/// Does this (already-pruned) value qualify for removal from its container?
fn removable(v: &Value, opts: &Options) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => opts.remove_empty_strings && s.is_empty(),
        Value::Array(a) => opts.remove_empty_arrays && a.is_empty(),
        Value::Object(m) => opts.remove_empty_objects && m.is_empty(),
        Value::Bool(_) | Value::Number(_) => false,
    }
}

fn render(value: &Value, indent: usize) -> Result<String, String> {
    let n = indent.min(8);
    if n == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize failed: {e}"));
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

    /// Minified defaults, so expected values stay readable in the assertions.
    fn min() -> Options {
        Options { indent: 0, ..Options::default() }
    }

    #[test]
    fn removes_null_keys_recursively() {
        let out =
            remove_nulls(r#"{"a":1,"b":null,"c":{"d":null,"e":"x"}}"#, min()).unwrap();
        assert_eq!(out, r#"{"a":1,"c":{"e":"x"}}"#);
    }

    #[test]
    fn preserves_key_order_and_falsy_values() {
        // false, 0 and "" are real values — untouched unless opted in.
        let out = remove_nulls(r#"{"z":false,"y":0,"x":"","w":null,"v":[]}"#, min()).unwrap();
        assert_eq!(out, r#"{"z":false,"y":0,"x":"","v":[]}"#);
    }

    #[test]
    fn arrays_compact_by_default() {
        let out = remove_nulls(r#"{"a":[1,null,2,null]}"#, min()).unwrap();
        assert_eq!(out, r#"{"a":[1,2]}"#);
    }

    #[test]
    fn arrays_keep_leaves_holes_but_prunes_nested_objects() {
        let opts = Options { arrays: Arrays::Keep, ..min() };
        let out = remove_nulls(r#"[1,null,{"a":null,"b":2}]"#, opts).unwrap();
        assert_eq!(out, r#"[1,null,{"b":2}]"#);
    }

    #[test]
    fn opt_in_empty_strings_arrays_objects() {
        let opts = Options {
            remove_empty_strings: true,
            remove_empty_arrays: true,
            remove_empty_objects: true,
            ..min()
        };
        let out = remove_nulls(r#"{"a":"","b":[],"c":{},"d":"keep","e":0}"#, opts).unwrap();
        assert_eq!(out, r#"{"d":"keep","e":0}"#);
    }

    #[test]
    fn empty_removal_cascades_bottom_up() {
        // {"b":{"c":null}} → {"b":{}} → {} → the "a" key goes too.
        let opts = Options { remove_empty_objects: true, ..min() };
        let out = remove_nulls(r#"{"a":{"b":{"c":null}},"keep":1}"#, opts).unwrap();
        assert_eq!(out, r#"{"keep":1}"#);
    }

    #[test]
    fn cascade_stops_when_empty_objects_not_opted_in() {
        let out = remove_nulls(r#"{"a":{"b":{"c":null}},"keep":1}"#, min()).unwrap();
        assert_eq!(out, r#"{"a":{"b":{}},"keep":1}"#);
    }

    #[test]
    fn trim_strings_makes_whitespace_only_removable() {
        let opts = Options { trim_strings: true, remove_empty_strings: true, ..min() };
        let out = remove_nulls(r#"{"a":"  hi  ","b":"   ","c":"\t\n"}"#, opts).unwrap();
        assert_eq!(out, r#"{"a":"hi"}"#);
    }

    #[test]
    fn trim_strings_alone_keeps_blanks() {
        let opts = Options { trim_strings: true, ..min() };
        let out = remove_nulls(r#"{"a":" hi ","b":"  "}"#, opts).unwrap();
        assert_eq!(out, r#"{"a":"hi","b":""}"#);
    }

    #[test]
    fn root_value_is_never_dropped() {
        assert_eq!(remove_nulls(r#"{"a":null}"#, min()).unwrap(), "{}");
        assert_eq!(remove_nulls("null", min()).unwrap(), "null");
        let opts = Options { remove_empty_objects: true, ..min() };
        assert_eq!(remove_nulls(r#"{"a":null}"#, opts).unwrap(), "{}");
    }

    #[test]
    fn array_of_objects_is_pruned_elementwise() {
        let out = remove_nulls(r#"[{"a":1,"b":null},{"a":null,"b":2}]"#, min()).unwrap();
        assert_eq!(out, r#"[{"a":1},{"b":2}]"#);
    }

    #[test]
    fn nested_empty_arrays_cascade_when_opted_in() {
        let opts = Options { remove_empty_arrays: true, ..min() };
        let out = remove_nulls(r#"{"a":[null,null],"b":[1]}"#, opts).unwrap();
        assert_eq!(out, r#"{"b":[1]}"#);
    }

    #[test]
    fn pretty_output_by_default_indent() {
        let out = remove_nulls(r#"{"a":1,"b":null}"#, Options::default()).unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn indent_clamped_to_eight() {
        let opts = Options { indent: 99, ..Options::default() };
        let out = remove_nulls(r#"{"a":1,"b":null}"#, opts).unwrap();
        assert_eq!(out, "{\n        \"a\": 1\n}");
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(remove_nulls("{bad}", Options::default()).is_err());
        assert!(remove_nulls("", Options::default()).is_err());
        assert!(remove_nulls("   ", Options::default()).is_err());
        // Trailing commas / unquoted keys are not repaired, they're rejected.
        assert!(remove_nulls("[1,2,]", Options::default()).is_err());
    }

    #[test]
    fn error_message_names_the_problem() {
        let err = remove_nulls("{bad}", Options::default()).unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "got {err}");
        assert!(err.contains("line 1"), "got {err}");
    }

    #[test]
    fn arrays_parse_accepts_aliases() {
        assert_eq!(Arrays::parse("keep"), Arrays::Keep);
        assert_eq!(Arrays::parse("  KEEP "), Arrays::Keep);
        assert_eq!(Arrays::parse("compact"), Arrays::Compact);
        assert_eq!(Arrays::parse(""), Arrays::Compact);
    }
}
