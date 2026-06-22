//! gizza-ai/json-diff core — structural diff of two JSON documents.
//! Walks both trees recursively and reports, per JSON-path location, whether a
//! value was added, removed or changed. Objects are compared key-by-key; arrays
//! are compared index-by-index. Pure-Rust (serde_json, preserve_order).

use serde::Serialize;
use serde_json::{Map, Value};

/// One structural change between the two documents.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Change {
    /// Dotted/bracketed JSON path of the location, e.g. `a.b` or `list[2]`. The
    /// root itself is reported as `$`.
    pub path: String,
    /// `"added"` (only in the second doc), `"removed"` (only in the first) or
    /// `"changed"` (present in both but different).
    pub kind: String,
    /// The value in the first (left/old) document, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Value>,
    /// The value in the second (right/new) document, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct DiffReport {
    equal: bool,
    added: usize,
    removed: usize,
    changed: usize,
    changes: Vec<Change>,
}

fn child_path(base: &str, key: &str) -> String {
    if base == "$" {
        // top-level object key
        format!("$.{key}")
    } else {
        format!("{base}.{key}")
    }
}

fn index_path(base: &str, idx: usize) -> String {
    format!("{base}[{idx}]")
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn diff_value(path: &str, a: &Value, b: &Value, out: &mut Vec<Change>) {
    match (a, b) {
        (Value::Object(am), Value::Object(bm)) => diff_objects(path, am, bm, out),
        (Value::Array(aa), Value::Array(bb)) => diff_arrays(path, aa, bb, out),
        _ => {
            // Same containerless comparison: if types differ OR scalar values
            // differ, it's a change. Equal values produce nothing.
            if a != b || type_name(a) != type_name(b) {
                out.push(Change {
                    path: path.to_string(),
                    kind: "changed".into(),
                    old: Some(a.clone()),
                    new: Some(b.clone()),
                });
            }
        }
    }
}

fn diff_objects(path: &str, a: &Map<String, Value>, b: &Map<String, Value>, out: &mut Vec<Change>) {
    // Removed / changed keys (iterate first doc, preserving its order).
    for (k, av) in a {
        let cp = child_path(path, k);
        match b.get(k) {
            None => out.push(Change {
                path: cp,
                kind: "removed".into(),
                old: Some(av.clone()),
                new: None,
            }),
            Some(bv) => diff_value(&cp, av, bv, out),
        }
    }
    // Added keys (present in second doc only).
    for (k, bv) in b {
        if !a.contains_key(k) {
            out.push(Change {
                path: child_path(path, k),
                kind: "added".into(),
                old: None,
                new: Some(bv.clone()),
            });
        }
    }
}

fn diff_arrays(path: &str, a: &[Value], b: &[Value], out: &mut Vec<Change>) {
    let common = a.len().min(b.len());
    for i in 0..common {
        diff_value(&index_path(path, i), &a[i], &b[i], out);
    }
    // Extra elements in the first doc were removed.
    for (i, av) in a.iter().enumerate().skip(common) {
        out.push(Change {
            path: index_path(path, i),
            kind: "removed".into(),
            old: Some(av.clone()),
            new: None,
        });
    }
    // Extra elements in the second doc were added.
    for (i, bv) in b.iter().enumerate().skip(common) {
        out.push(Change {
            path: index_path(path, i),
            kind: "added".into(),
            old: None,
            new: Some(bv.clone()),
        });
    }
}

/// Diff two JSON documents and return a JSON report describing the structural
/// differences. `indent` controls the output indentation (0 = minified).
///
/// The report has the shape:
/// `{ "equal": bool, "added": N, "removed": N, "changed": N, "changes": [ { "path", "kind", "old"?, "new"? }, ... ] }`.
pub fn diff_documents(left: &str, right: &str, indent: usize) -> Result<String, String> {
    if left.trim().is_empty() {
        return Err("the first (left) JSON input is empty".into());
    }
    if right.trim().is_empty() {
        return Err("the second (right) JSON input is empty".into());
    }
    let a: Value = serde_json::from_str(left).map_err(|e| format!("the first (left) JSON is invalid: {e}"))?;
    let b: Value = serde_json::from_str(right).map_err(|e| format!("the second (right) JSON is invalid: {e}"))?;

    let mut changes = Vec::new();
    diff_value("$", &a, &b, &mut changes);

    let added = changes.iter().filter(|c| c.kind == "added").count();
    let removed = changes.iter().filter(|c| c.kind == "removed").count();
    let changed = changes.iter().filter(|c| c.kind == "changed").count();
    let report = DiffReport {
        equal: changes.is_empty(),
        added,
        removed,
        changed,
        changes,
    };

    write_pretty(&report, indent)
}

fn write_pretty<T: Serialize>(value: &T, indent: usize) -> Result<String, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report(left: &str, right: &str) -> Value {
        serde_json::from_str(&diff_documents(left, right, 0).unwrap()).unwrap()
    }

    #[test]
    fn identical_documents_are_equal() {
        let r = report(r#"{"a":1,"b":[1,2]}"#, r#"{"a":1,"b":[1,2]}"#);
        assert_eq!(r["equal"], true);
        assert_eq!(r["added"], 0);
        assert_eq!(r["removed"], 0);
        assert_eq!(r["changed"], 0);
        assert_eq!(r["changes"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn detects_added_and_removed_keys() {
        let r = report(r#"{"a":1,"gone":2}"#, r#"{"a":1,"new":3}"#);
        assert_eq!(r["equal"], false);
        assert_eq!(r["added"], 1);
        assert_eq!(r["removed"], 1);
        let changes = r["changes"].as_array().unwrap();
        // removed reported before added (first-doc order, then added keys)
        assert!(changes.iter().any(|c| c["path"] == "$.gone" && c["kind"] == "removed" && c["old"] == 2));
        assert!(changes.iter().any(|c| c["path"] == "$.new" && c["kind"] == "added" && c["new"] == 3));
    }

    #[test]
    fn detects_changed_scalar() {
        let r = report(r#"{"a":1}"#, r#"{"a":2}"#);
        assert_eq!(r["changed"], 1);
        let c = &r["changes"][0];
        assert_eq!(c["path"], "$.a");
        assert_eq!(c["kind"], "changed");
        assert_eq!(c["old"], 1);
        assert_eq!(c["new"], 2);
    }

    #[test]
    fn type_change_is_a_change() {
        let r = report(r#"{"a":1}"#, r#"{"a":"1"}"#);
        assert_eq!(r["changed"], 1);
        assert_eq!(r["changes"][0]["old"], json!(1));
        assert_eq!(r["changes"][0]["new"], json!("1"));
    }

    #[test]
    fn nested_object_paths() {
        let r = report(r#"{"a":{"b":{"c":1}}}"#, r#"{"a":{"b":{"c":2}}}"#);
        assert_eq!(r["changes"][0]["path"], "$.a.b.c");
        assert_eq!(r["changes"][0]["kind"], "changed");
    }

    #[test]
    fn array_index_changes() {
        let r = report(r#"{"l":[1,2,3]}"#, r#"{"l":[1,9]}"#);
        let changes = r["changes"].as_array().unwrap();
        // index 1 changed, index 2 removed
        assert!(changes.iter().any(|c| c["path"] == "$.l[1]" && c["kind"] == "changed"));
        assert!(changes.iter().any(|c| c["path"] == "$.l[2]" && c["kind"] == "removed" && c["old"] == 3));
    }

    #[test]
    fn array_growth_reports_added() {
        let r = report(r#"[1]"#, r#"[1,2,3]"#);
        let changes = r["changes"].as_array().unwrap();
        assert!(changes.iter().any(|c| c["path"] == "$[1]" && c["kind"] == "added" && c["new"] == 2));
        assert!(changes.iter().any(|c| c["path"] == "$[2]" && c["kind"] == "added" && c["new"] == 3));
    }

    #[test]
    fn root_scalar_change() {
        let r = report("1", "2");
        assert_eq!(r["changes"][0]["path"], "$");
        assert_eq!(r["changes"][0]["kind"], "changed");
    }

    #[test]
    fn root_container_type_change() {
        let r = report(r#"{"a":1}"#, r#"[1]"#);
        assert_eq!(r["changed"], 1);
        assert_eq!(r["changes"][0]["path"], "$");
    }

    #[test]
    fn pretty_output_has_newlines() {
        let out = diff_documents(r#"{"a":1}"#, r#"{"a":2}"#, 2).unwrap();
        assert!(out.contains('\n'));
        assert!(out.contains("\"equal\": false"));
    }

    #[test]
    fn errors_on_empty_and_invalid() {
        assert!(diff_documents("", "{}", 0).is_err());
        assert!(diff_documents("{}", "", 0).is_err());
        assert!(diff_documents("{bad}", "{}", 0).is_err());
        assert!(diff_documents("{}", "{oops}", 0).is_err());
    }
}
