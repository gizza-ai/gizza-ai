//! gizza-ai/json-sort core — recursively sort JSON object keys (and, optionally,
//! array elements) to produce stable, diff-friendly output. The input is parsed
//! and validated first, so malformed JSON is rejected with the parser's exact
//! line/column. Pure-Rust; shared by the chat skill block and the web page.

use serde::Serialize;
use serde_json::{Map, Value};

/// Sort direction for keys (and array elements when `sort_arrays` is on).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    /// Parse the `order` param. Blank/unknown falls back to ascending.
    pub fn parse(s: &str) -> Order {
        match s.trim().to_ascii_lowercase().as_str() {
            "desc" | "descending" | "z-a" | "za" => Order::Desc,
            _ => Order::Asc,
        }
    }
}

/// Options controlling the sort.
#[derive(Clone, Copy)]
pub struct Options {
    /// Ascending (A→Z) or descending (Z→A) order.
    pub order: Order,
    /// Also sort the elements of every array (by their serialized form),
    /// not just object keys. Off by default so array order stays meaningful.
    pub sort_arrays: bool,
    /// Compare keys case-insensitively (`Name` next to `name`) instead of the
    /// default codepoint/ASCII order where uppercase sorts before lowercase.
    pub case_insensitive: bool,
    /// Spaces of indentation per level (clamped 0..=8). `0` minifies to one
    /// compact line.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options { order: Order::Asc, sort_arrays: false, case_insensitive: false, indent: 2 }
    }
}

/// Parse, recursively sort, and re-serialize `json`. Returns a validation error
/// with line/column if the input isn't valid JSON.
pub fn sort(json: &str, opts: Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON input".into());
    }
    let mut value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    sort_value(&mut value, &opts);

    let n = opts.indent.min(8);
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

/// Case-insensitive optional key comparator with a case-sensitive tie-break so
/// the result is fully deterministic even for keys differing only in case.
fn key_cmp(a: &str, b: &str, opts: &Options) -> std::cmp::Ordering {
    let primary = if opts.case_insensitive {
        a.to_lowercase().cmp(&b.to_lowercase())
    } else {
        std::cmp::Ordering::Equal
    };
    let ord = primary.then_with(|| a.cmp(b));
    match opts.order {
        Order::Asc => ord,
        Order::Desc => ord.reverse(),
    }
}

fn sort_value(value: &mut Value, opts: &Options) {
    match value {
        Value::Object(map) => {
            // Recurse first so nested containers are sorted before we reorder.
            for (_, v) in map.iter_mut() {
                sort_value(v, opts);
            }
            let mut entries: Vec<(String, Value)> =
                std::mem::replace(map, Map::new()).into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| key_cmp(a, b, opts));
            for (k, v) in entries {
                map.insert(k, v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                sort_value(v, opts);
            }
            if opts.sort_arrays {
                // Sort by each element's canonical compact serialization so the
                // ordering is total and deterministic across mixed types.
                arr.sort_by(|a, b| {
                    let ka = serde_json::to_string(a).unwrap_or_default();
                    let kb = serde_json::to_string(b).unwrap_or_default();
                    match opts.order {
                        Order::Asc => ka.cmp(&kb),
                        Order::Desc => ka.cmp(&kb).reverse(),
                    }
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(order: Order, sort_arrays: bool, case_insensitive: bool, indent: usize) -> Options {
        Options { order, sort_arrays, case_insensitive, indent }
    }

    #[test]
    fn sorts_keys_recursively_ascending() {
        let out = sort(r#"{"b":1,"a":{"z":2,"y":3}}"#, opts(Order::Asc, false, false, 0)).unwrap();
        assert_eq!(out, r#"{"a":{"y":3,"z":2},"b":1}"#);
    }

    #[test]
    fn descending_order() {
        let out = sort(r#"{"a":1,"b":2,"c":3}"#, opts(Order::Desc, false, false, 0)).unwrap();
        assert_eq!(out, r#"{"c":3,"b":2,"a":1}"#);
    }

    #[test]
    fn array_order_preserved_by_default() {
        // Object keys inside array elements get sorted, but element order stays.
        let out = sort(r#"[{"b":1,"a":2},3,1,2]"#, opts(Order::Asc, false, false, 0)).unwrap();
        assert_eq!(out, r#"[{"a":2,"b":1},3,1,2]"#);
    }

    #[test]
    fn sort_arrays_enabled() {
        let out = sort(r#"{"nums":[3,1,2],"strs":["b","a","c"]}"#, opts(Order::Asc, true, false, 0)).unwrap();
        assert_eq!(out, r#"{"nums":[1,2,3],"strs":["a","b","c"]}"#);
    }

    #[test]
    fn case_sensitive_default_uppercase_first() {
        // Default codepoint order: uppercase 'B' (0x42) sorts before lowercase 'a' (0x61).
        let out = sort(r#"{"a":1,"B":2}"#, opts(Order::Asc, false, false, 0)).unwrap();
        assert_eq!(out, r#"{"B":2,"a":1}"#);
    }

    #[test]
    fn case_insensitive_grouping() {
        let out = sort(r#"{"banana":1,"Apple":2,"cherry":3}"#, opts(Order::Asc, false, true, 0)).unwrap();
        assert_eq!(out, r#"{"Apple":2,"banana":1,"cherry":3}"#);
    }

    #[test]
    fn pretty_indent_two() {
        let out = sort(r#"{"b":1,"a":2}"#, opts(Order::Asc, false, false, 2)).unwrap();
        assert_eq!(out, "{\n  \"a\": 2,\n  \"b\": 1\n}");
    }

    #[test]
    fn scalar_root_passthrough() {
        assert_eq!(sort("42", Options::default()).unwrap(), "42");
        assert_eq!(sort(r#""hi""#, Options::default()).unwrap(), r#""hi""#);
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(sort("{bad}", Options::default()).is_err());
        assert!(sort("", Options::default()).is_err());
        assert!(sort("[1,2,]", Options::default()).is_err()); // trailing comma
    }

    #[test]
    fn indent_clamped_to_eight() {
        let out = sort(r#"{"a":1}"#, opts(Order::Asc, false, false, 99)).unwrap();
        // 8 spaces of indentation, not 99.
        assert_eq!(out, "{\n        \"a\": 1\n}");
    }
}
