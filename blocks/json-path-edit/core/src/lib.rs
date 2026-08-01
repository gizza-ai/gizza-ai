//! json-path-edit core — get, set, or delete a value at a dotted or bracketed
//! path in a JSON document. Pure compute, shared by the chat skill block, the
//! CLI, and the web page. No wafer/wasm-bindgen deps and no host/WASI calls, so
//! it runs in the gizza wafer runtime (chat SW), the CLI, and the browser page
//! alike.
//!
//! Path notation (lodash / `dot-object` style, NOT RFC 9535 JSONPath):
//!   - dot segments:      `store.book.title`
//!   - array indices:     `store.book[0]` or the equivalent `store.book.0`
//!   - quoted keys:       `store["book list"].0` / `store['a.b'].c` (a key that
//!                        itself contains `.`, `[`, or spaces)
//!   - an optional leading `$` root marker is accepted and ignored (`$.a.b`).
//!
//! Operations:
//!   - `get`    — return the value at the path (serialized JSON).
//!   - `set`    — set the value at the path, creating intermediate objects and
//!                arrays as needed; returns the whole modified document.
//!   - `delete` — remove the key / array element at the path; returns the whole
//!                modified document.

use serde_json::{Map, Value};

/// Cap on how far a `set` may grow an array by index, so a typo like `a[999999999]`
/// can't allocate a giant array. Setting an index above this errors instead.
pub const MAX_ARRAY_GROW: usize = 100_000;

/// One parsed path segment.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    key: String,
    /// True when the segment came from a *quoted* bracket (`["0"]`), which forces
    /// it to be treated as an object key even if it looks numeric.
    quoted: bool,
}

/// Parse a dotted / bracketed path into segments.
fn parse_path(path: &str) -> Result<Vec<Segment>, String> {
    let s = path.trim();
    // Optional leading `$` root marker (`$`, `$.a`, `$['a']`).
    let s = s.strip_prefix('$').unwrap_or(s);
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut segs: Vec<Segment> = Vec::new();
    let mut i = 0;
    while i < n {
        match chars[i] {
            // Separator (also swallows a leading '.').
            '.' => {
                i += 1;
            }
            '[' => {
                i += 1;
                if i < n && (chars[i] == '"' || chars[i] == '\'') {
                    let quote = chars[i];
                    i += 1;
                    let mut buf = String::new();
                    let mut closed = false;
                    while i < n {
                        if chars[i] == '\\' && i + 1 < n {
                            // Allow escaping the quote (and backslash) inside a key.
                            buf.push(chars[i + 1]);
                            i += 2;
                            continue;
                        }
                        if chars[i] == quote {
                            closed = true;
                            i += 1;
                            break;
                        }
                        buf.push(chars[i]);
                        i += 1;
                    }
                    if !closed {
                        return Err(format!("unterminated quoted key in path near '{buf}'"));
                    }
                    if i >= n || chars[i] != ']' {
                        return Err("expected ']' after a quoted key in path".into());
                    }
                    i += 1;
                    segs.push(Segment { key: buf, quoted: true });
                } else {
                    let mut buf = String::new();
                    while i < n && chars[i] != ']' {
                        buf.push(chars[i]);
                        i += 1;
                    }
                    if i >= n {
                        return Err("unterminated '[' in path (missing ']')".into());
                    }
                    i += 1;
                    let key = buf.trim().to_string();
                    if key.is_empty() {
                        return Err("empty '[]' in path".into());
                    }
                    segs.push(Segment { key, quoted: false });
                }
            }
            ']' => return Err("unexpected ']' in path".into()),
            _ => {
                let mut buf = String::new();
                while i < n && chars[i] != '.' && chars[i] != '[' {
                    buf.push(chars[i]);
                    i += 1;
                }
                if !buf.is_empty() {
                    segs.push(Segment { key: buf, quoted: false });
                }
            }
        }
    }
    Ok(segs)
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

/// Parse a segment as a usize array index (only when it wasn't a quoted key).
fn as_index(seg: &Segment) -> Option<usize> {
    if seg.quoted {
        return None;
    }
    seg.key.parse::<usize>().ok()
}

/// Traverse to the value at `segs`, returning a reference. Errors if any segment
/// is missing or the container type doesn't match the segment.
fn resolve<'a>(root: &'a Value, segs: &[Segment]) -> Result<&'a Value, String> {
    let mut cur = root;
    for seg in segs {
        match cur {
            Value::Object(map) => {
                cur = map.get(&seg.key).ok_or_else(|| {
                    format!("no value at path: object has no key '{}'", seg.key)
                })?;
            }
            Value::Array(arr) => {
                let idx = as_index(seg)
                    .ok_or_else(|| format!("expected an array index, got '{}'", seg.key))?;
                cur = arr.get(idx).ok_or_else(|| {
                    format!("array index {idx} out of bounds (length {})", arr.len())
                })?;
            }
            other => {
                return Err(format!(
                    "cannot descend into a {} at '{}'",
                    type_name(other),
                    seg.key
                ));
            }
        }
    }
    Ok(cur)
}

/// Recursively set `newval` at `segs` (non-empty), creating intermediates.
fn set_rec(cur: &mut Value, segs: &[Segment], newval: Value) -> Result<(), String> {
    let seg = &segs[0];
    let rest = &segs[1..];
    let numeric = as_index(seg).is_some();

    // An empty slot (null) becomes an array or object depending on this segment.
    if cur.is_null() {
        *cur = if numeric {
            Value::Array(Vec::new())
        } else {
            Value::Object(Map::new())
        };
    }

    match cur {
        Value::Object(map) => {
            if rest.is_empty() {
                map.insert(seg.key.clone(), newval);
            } else {
                let child = map.entry(seg.key.clone()).or_insert(Value::Null);
                set_rec(child, rest, newval)?;
            }
        }
        Value::Array(arr) => {
            let idx = as_index(seg)
                .ok_or_else(|| format!("expected an array index, got '{}'", seg.key))?;
            if idx >= arr.len() {
                if idx >= MAX_ARRAY_GROW {
                    return Err(format!(
                        "array index {idx} exceeds the maximum of {}",
                        MAX_ARRAY_GROW - 1
                    ));
                }
                arr.resize(idx + 1, Value::Null);
            }
            if rest.is_empty() {
                arr[idx] = newval;
            } else {
                set_rec(&mut arr[idx], rest, newval)?;
            }
        }
        other => {
            return Err(format!(
                "cannot set into a {} at '{}' — the existing value is not an object or array",
                type_name(other),
                seg.key
            ));
        }
    }
    Ok(())
}

/// Recursively delete the value at `segs` (non-empty).
fn delete_rec(cur: &mut Value, segs: &[Segment]) -> Result<(), String> {
    let seg = &segs[0];
    let rest = &segs[1..];
    match cur {
        Value::Object(map) => {
            if rest.is_empty() {
                if map.remove(&seg.key).is_none() {
                    return Err(format!("nothing to delete: object has no key '{}'", seg.key));
                }
            } else {
                let child = map.get_mut(&seg.key).ok_or_else(|| {
                    format!("no value at path: object has no key '{}'", seg.key)
                })?;
                delete_rec(child, rest)?;
            }
        }
        Value::Array(arr) => {
            let idx = as_index(seg)
                .ok_or_else(|| format!("expected an array index, got '{}'", seg.key))?;
            if idx >= arr.len() {
                return Err(format!(
                    "array index {idx} out of bounds (length {})",
                    arr.len()
                ));
            }
            if rest.is_empty() {
                arr.remove(idx);
            } else {
                delete_rec(&mut arr[idx], rest)?;
            }
        }
        other => {
            return Err(format!(
                "cannot delete from a {} at '{}'",
                type_name(other),
                seg.key
            ));
        }
    }
    Ok(())
}

/// Parse `value` as JSON; if it isn't valid JSON, treat it as a plain string.
/// So `42` → number, `true` → boolean, `null` → null, `{"a":1}` → object,
/// `"x"` → the string "x", and a bare `hello` → the string "hello".
fn parse_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
}

fn serialize(v: &Value, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(v)
    } else {
        serde_json::to_string(v)
    }
    .map_err(|e| format!("serialize error: {e}"))
}

/// Get, set, or delete the value at `path` in the JSON document `json`.
///
/// - `operation`: `"get"`, `"set"`, or `"delete"`.
/// - `value`: for `set`, the value to write (parsed as JSON, falling back to a
///   string literal — wrap in quotes to force a string). Ignored for get/delete.
/// - `pretty`: pretty-print (indent) the output. `get` returns the value at the
///   path; `set`/`delete` return the whole modified document.
pub fn edit(
    json: &str,
    path: &str,
    operation: &str,
    value: &str,
    pretty: bool,
) -> Result<String, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON document: {e}"))?;
    let segs = parse_path(path)?;

    let op = operation.trim().to_ascii_lowercase();
    // An omitted operation defaults to "get" (matching the descriptor default).
    let op = if op.is_empty() { "get" } else { op.as_str() };
    match op {
        "get" => {
            if segs.is_empty() {
                return serialize(&root, pretty);
            }
            let found = resolve(&root, &segs)?;
            serialize(found, pretty)
        }
        "set" => {
            let mut root = root;
            let newval = parse_value(value);
            if segs.is_empty() {
                // Setting the whole document to the new value.
                return serialize(&newval, pretty);
            }
            set_rec(&mut root, &segs, newval)?;
            serialize(&root, pretty)
        }
        "delete" => {
            let mut root = root;
            if segs.is_empty() {
                return Err(
                    "path is empty — nothing to delete (the path selects the whole document)"
                        .into(),
                );
            }
            delete_rec(&mut root, &segs)?;
            serialize(&root, pretty)
        }
        other => Err(format!(
            "unknown operation '{other}' — expected 'get', 'set', or 'delete'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str =
        r#"{"store":{"book":[{"title":"A","price":5},{"title":"B","price":12}],"open":true}}"#;

    #[test]
    fn get_nested_scalar() {
        assert_eq!(
            edit(DOC, "store.book[0].title", "get", "", false).unwrap(),
            "\"A\""
        );
        // dotted array index is equivalent to bracket index
        assert_eq!(edit(DOC, "store.book.1.price", "get", "", false).unwrap(), "12");
        // scalar subtree
        assert_eq!(edit(DOC, "store.open", "get", "", false).unwrap(), "true");
    }

    #[test]
    fn empty_operation_defaults_to_get() {
        // The CLI/page pass an empty string when `operation` is omitted; that must
        // behave like the descriptor default of "get".
        assert_eq!(edit(DOC, "store.open", "", "", false).unwrap(), "true");
    }

    #[test]
    fn get_quoted_key_with_dot() {
        let doc = r#"{"a.b":{"c":1}}"#;
        assert_eq!(edit(doc, "[\"a.b\"].c", "get", "", false).unwrap(), "1");
    }

    #[test]
    fn set_existing_and_create_intermediate() {
        // overwrite existing
        let out = edit(DOC, "store.book[0].price", "set", "9", false).unwrap();
        assert!(out.contains(r#"{"title":"A","price":9}"#), "{out}");
        // create a nested path that doesn't exist yet
        let out = edit("{}", "a.b.c", "set", "\"hi\"", false).unwrap();
        assert_eq!(out, r#"{"a":{"b":{"c":"hi"}}}"#);
        // numeric segment creates an array
        let out = edit("{}", "list[2]", "set", "7", false).unwrap();
        assert_eq!(out, r#"{"list":[null,null,7]}"#);
    }

    #[test]
    fn set_value_type_inference() {
        assert_eq!(edit("{}", "x", "set", "true", false).unwrap(), r#"{"x":true}"#);
        assert_eq!(edit("{}", "x", "set", "null", false).unwrap(), r#"{"x":null}"#);
        assert_eq!(edit("{}", "x", "set", "hello", false).unwrap(), r#"{"x":"hello"}"#);
        assert_eq!(
            edit("{}", "x", "set", r#"{"k":1}"#, false).unwrap(),
            r#"{"x":{"k":1}}"#
        );
    }

    #[test]
    fn delete_object_key_and_array_element() {
        let out = edit(DOC, "store.open", "delete", "", false).unwrap();
        assert!(!out.contains("open"), "{out}");
        // deleting an array element shifts the rest down
        let out = edit(DOC, "store.book[0]", "delete", "", false).unwrap();
        assert_eq!(
            out,
            r#"{"store":{"book":[{"title":"B","price":12}],"open":true}}"#
        );
    }

    #[test]
    fn pretty_output() {
        let out = edit("{\"a\":1}", "a", "get", "", true).unwrap();
        assert_eq!(out, "1");
        let out = edit("{}", "a", "set", "1", true).unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn err_invalid_json() {
        let e = edit("{not json}", "a", "get", "", false).unwrap_err();
        assert!(e.contains("invalid JSON document"), "{e}");
    }

    #[test]
    fn err_get_missing_key() {
        let e = edit(DOC, "store.nope", "get", "", false).unwrap_err();
        assert!(e.contains("no value at path"), "{e}");
    }

    #[test]
    fn err_index_out_of_bounds() {
        let e = edit(DOC, "store.book[9]", "get", "", false).unwrap_err();
        assert!(e.contains("out of bounds"), "{e}");
    }

    #[test]
    fn err_set_into_scalar() {
        let e = edit(r#"{"a":5}"#, "a.b", "set", "1", false).unwrap_err();
        assert!(e.contains("not an object or array"), "{e}");
    }

    #[test]
    fn err_unknown_operation() {
        let e = edit("{}", "a", "frobnicate", "", false).unwrap_err();
        assert!(e.contains("unknown operation"), "{e}");
    }

    #[test]
    fn err_array_grow_cap() {
        let e = edit("{}", "a[999999]", "set", "1", false).unwrap_err();
        assert!(e.contains("maximum"), "{e}");
    }
}
