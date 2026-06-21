//! gizza-ai/json-merge core — deep-merge two or more JSON values. Objects are
//! merged recursively (later documents win on scalar/type conflicts); arrays are
//! either replaced (default) or concatenated. Pure-Rust (serde_json,
//! preserve_order).

use serde::Serialize;
use serde_json::Value;

/// Deep-merge `b` into `a`. Objects merge key-by-key; arrays concat when
/// `array_concat`, else `b` replaces `a`; otherwise `b` wins.
fn merge(a: Value, b: Value, array_concat: bool) -> Value {
    match (a, b) {
        (Value::Object(mut am), Value::Object(bm)) => {
            for (k, bv) in bm {
                let merged = match am.remove(&k) {
                    Some(av) => merge(av, bv, array_concat),
                    None => bv,
                };
                am.insert(k, merged);
            }
            Value::Object(am)
        }
        (Value::Array(mut aa), Value::Array(bb)) if array_concat => {
            aa.extend(bb);
            Value::Array(aa)
        }
        (_, b) => b,
    }
}

fn write_pretty(value: &Value, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize: {e}"));
    }
    let pad = vec![b' '; indent.min(8)];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value.serialize(&mut ser).map_err(|e| format!("serialize: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

/// Merge all JSON values found in `documents` (whitespace/newline-separated,
/// concatenated JSON values). `array_concat` controls array handling; `indent`
/// is the output indentation (0 = minified).
pub fn merge_documents(documents: &str, array_concat: bool, indent: usize) -> Result<String, String> {
    if documents.trim().is_empty() {
        return Err("no JSON input".into());
    }
    let iter = serde_json::Deserializer::from_str(documents).into_iter::<Value>();
    let mut values: Vec<Value> = Vec::new();
    for (i, r) in iter.enumerate() {
        let v = r.map_err(|e| format!("JSON value #{} is invalid: {e}", i + 1))?;
        values.push(v);
    }
    if values.is_empty() {
        return Err("no JSON values found".into());
    }
    let mut acc = values.remove(0);
    for v in values {
        acc = merge(acc, v, array_concat);
    }
    write_pretty(&acc, indent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_merges_objects() {
        let out = merge_documents(r#"{"a":1,"nested":{"x":1}} {"b":2,"nested":{"y":2}}"#, false, 0).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
        assert_eq!(v["nested"]["x"], 1);
        assert_eq!(v["nested"]["y"], 2);
    }

    #[test]
    fn later_wins_on_scalar_conflict() {
        let out = merge_documents(r#"{"a":1} {"a":99}"#, false, 0).unwrap();
        assert_eq!(out, r#"{"a":99}"#);
    }

    #[test]
    fn arrays_replace_by_default() {
        let out = merge_documents(r#"{"l":[1,2]} {"l":[3]}"#, false, 0).unwrap();
        assert_eq!(out, r#"{"l":[3]}"#);
    }

    #[test]
    fn arrays_concat_when_enabled() {
        let out = merge_documents(r#"{"l":[1,2]} {"l":[3]}"#, true, 0).unwrap();
        assert_eq!(out, r#"{"l":[1,2,3]}"#);
    }

    #[test]
    fn three_documents_newline_separated() {
        let out = merge_documents("{\"a\":1}\n{\"b\":2}\n{\"c\":3}", false, 0).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["c"], 3);
    }

    #[test]
    fn single_document_is_formatted() {
        let out = merge_documents(r#"{"a":1}"#, false, 2).unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn errors() {
        assert!(merge_documents("", false, 0).is_err());
        assert!(merge_documents("{bad}", false, 0).is_err());
        assert!(merge_documents("{\"a\":1} {oops}", false, 0).is_err());
    }
}
