//! gizza-ai/sort-json-array core — sort a JSON **array of objects** by one or
//! more fields: comma-separated keys, nested dot-paths (`user.name`, array index
//! `tags.0`), a per-key `+`/`-` direction prefix, a global asc/desc default, and
//! configurable placement of missing/null values. This sorts array ELEMENTS by a
//! chosen field — distinct from the `json-sort` tool, which reorders object keys.
//! Pure-Rust; shared by the chat skill block and the web page.

use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;

/// Sort direction — the global default and each key's resolved direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// Where rows whose sort field is absent or JSON `null` are placed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Missing {
    First,
    Last,
}

impl Missing {
    /// Parse the `missing` param. Blank/unknown falls back to last.
    pub fn parse(s: &str) -> Missing {
        match s.trim().to_ascii_lowercase().as_str() {
            "first" => Missing::First,
            _ => Missing::Last,
        }
    }
}

/// Options controlling the array sort.
pub struct Options {
    /// Comma-separated sort keys. Each key may use dot-notation for nested paths
    /// (`address.city`, array index `tags.0`) and an optional `-` (descending) or
    /// `+` (ascending) prefix that overrides `order` for that key.
    pub keys: String,
    /// Global direction default for keys without an explicit `+`/`-` prefix.
    pub order: Order,
    /// Placement of elements whose field is missing or `null`.
    pub missing: Missing,
    /// Compare string values case-insensitively.
    pub case_insensitive: bool,
    /// Spaces of indentation per level (clamped 0..=8). `0` minifies to one line.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            keys: String::new(),
            order: Order::Asc,
            missing: Missing::Last,
            case_insensitive: false,
            indent: 2,
        }
    }
}

/// One resolved sort key: a dot-split path plus its direction.
struct SortKey {
    path: Vec<String>,
    order: Order,
}

/// Parse a comma-separated `keys` spec into ordered `SortKey`s. Each entry may
/// carry a leading `-` (descending) or `+` (ascending) that overrides `global`.
fn parse_keys(spec: &str, global: Order) -> Result<Vec<SortKey>, String> {
    let mut out = Vec::new();
    for raw in spec.split(',') {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        let (order, rest) = if let Some(r) = t.strip_prefix('-') {
            (Order::Desc, r.trim())
        } else if let Some(r) = t.strip_prefix('+') {
            (Order::Asc, r.trim())
        } else {
            (global, t)
        };
        if rest.is_empty() {
            return Err(format!("empty sort key in '{spec}'"));
        }
        let path: Vec<String> = rest.split('.').map(|s| s.to_string()).collect();
        out.push(SortKey { path, order });
    }
    if out.is_empty() {
        return Err("no sort keys given — set 'keys' to one or more field names".into());
    }
    Ok(out)
}

/// Resolve a dot-path inside one element. Returns `None` if any segment is
/// absent (object key missing, array index out of range, or descending into a
/// scalar).
fn resolve<'a>(v: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path {
        match cur {
            Value::Object(m) => cur = m.get(seg)?,
            Value::Array(a) => {
                let idx: usize = seg.parse().ok()?;
                cur = a.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

/// A field counts as "missing" when it is absent or explicitly JSON `null`.
fn is_missing(v: Option<&Value>) -> bool {
    matches!(v, None | Some(Value::Null))
}

/// Type rank for a total order across mixed JSON types (nulls are handled as
/// missing before this is reached).
fn type_rank(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

/// Compare two present field values. Numbers compare numerically, strings
/// lexicographically (optionally case-insensitively), booleans false<true, and
/// anything mixed/compound falls back to a stable type-rank then serialized-form
/// order. Direction is NOT applied here — the caller reverses for descending.
fn cmp_values(a: &Value, b: &Value, ci: bool) -> Ordering {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let xf = x.as_f64().unwrap_or(f64::NAN);
            let yf = y.as_f64().unwrap_or(f64::NAN);
            xf.partial_cmp(&yf).unwrap_or(Ordering::Equal)
        }
        (Value::String(x), Value::String(y)) => {
            if ci {
                x.to_lowercase()
                    .cmp(&y.to_lowercase())
                    .then_with(|| x.cmp(y))
            } else {
                x.cmp(y)
            }
        }
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => {
            let (ra, rb) = (type_rank(a), type_rank(b));
            if ra != rb {
                ra.cmp(&rb)
            } else {
                // Same compound type (array/object) — compare canonical form.
                let sa = serde_json::to_string(a).unwrap_or_default();
                let sb = serde_json::to_string(b).unwrap_or_default();
                sa.cmp(&sb)
            }
        }
    }
}

/// Compare two array elements across every sort key in turn (stable secondary
/// sort). Missing placement is absolute — independent of the key's direction.
fn compare_elems(a: &Value, b: &Value, keys: &[SortKey], opts: &Options) -> Ordering {
    for k in keys {
        let va = resolve(a, &k.path);
        let vb = resolve(b, &k.path);
        let ord = match (is_missing(va), is_missing(vb)) {
            (true, true) => Ordering::Equal,
            (true, false) => match opts.missing {
                Missing::First => Ordering::Less,
                Missing::Last => Ordering::Greater,
            },
            (false, true) => match opts.missing {
                Missing::First => Ordering::Greater,
                Missing::Last => Ordering::Less,
            },
            (false, false) => {
                let c = cmp_values(va.unwrap(), vb.unwrap(), opts.case_insensitive);
                match k.order {
                    Order::Asc => c,
                    Order::Desc => c.reverse(),
                }
            }
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// Serialize a value with `indent` spaces per level (`0` = minified).
fn serialize(value: &Value, indent: usize) -> Result<String, String> {
    let n = indent.min(8);
    if n == 0 {
        return serde_json::to_string(value).map_err(|e| format!("serialize failed: {e}"));
    }
    let pad = vec![b' '; n];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

/// Parse `json` (must be a JSON array), stably sort its elements by the parsed
/// `keys`, and re-serialize. Object keys inside each element are left untouched.
pub fn sort(json: &str, opts: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON input".into());
    }
    let keys = parse_keys(&opts.keys, opts.order)?;
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut arr = match value {
        Value::Array(a) => a,
        _ => return Err("input must be a JSON array of objects".into()),
    };
    arr.sort_by(|a, b| compare_elems(a, b, &keys, opts));
    serialize(&Value::Array(arr), opts.indent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(keys: &str, order: Order, missing: Missing, ci: bool, indent: usize) -> Options {
        Options {
            keys: keys.into(),
            order,
            missing,
            case_insensitive: ci,
            indent,
        }
    }

    #[test]
    fn sorts_by_single_numeric_key_ascending() {
        let out = sort(
            r#"[{"n":3},{"n":1},{"n":2}]"#,
            &opts("n", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":1},{"n":2},{"n":3}]"#);
    }

    #[test]
    fn numeric_sort_is_not_lexicographic() {
        // 10 must come after 9, not before (would happen with string compare).
        let out = sort(
            r#"[{"n":10},{"n":9},{"n":100}]"#,
            &opts("n", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":9},{"n":10},{"n":100}]"#);
    }

    #[test]
    fn global_descending() {
        let out = sort(
            r#"[{"n":1},{"n":3},{"n":2}]"#,
            &opts("n", Order::Desc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":3},{"n":2},{"n":1}]"#);
    }

    #[test]
    fn multi_key_with_per_key_direction() {
        // Sort by dept asc, then salary desc within each dept.
        let out = sort(
            r#"[{"dept":"b","salary":1},{"dept":"a","salary":1},{"dept":"a","salary":9}]"#,
            &opts("dept,-salary", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"dept":"a","salary":9},{"dept":"a","salary":1},{"dept":"b","salary":1}]"#
        );
    }

    #[test]
    fn plus_prefix_overrides_global_desc() {
        // Global desc, but +name forces that key ascending.
        let out = sort(
            r#"[{"name":"c"},{"name":"a"},{"name":"b"}]"#,
            &opts("+name", Order::Desc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"name":"a"},{"name":"b"},{"name":"c"}]"#);
    }

    #[test]
    fn nested_dot_path() {
        let out = sort(
            r#"[{"u":{"age":30}},{"u":{"age":10}},{"u":{"age":20}}]"#,
            &opts("u.age", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"u":{"age":10}},{"u":{"age":20}},{"u":{"age":30}}]"#
        );
    }

    #[test]
    fn array_index_path() {
        // Sort by the first element of each row's "pt" array.
        let out = sort(
            r#"[{"pt":[3,0]},{"pt":[1,9]},{"pt":[2,5]}]"#,
            &opts("pt.0", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"pt":[1,9]},{"pt":[2,5]},{"pt":[3,0]}]"#);
    }

    #[test]
    fn missing_last_by_default() {
        let out = sort(
            r#"[{"n":2},{"x":1},{"n":1}]"#,
            &opts("n", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":1},{"n":2},{"x":1}]"#);
    }

    #[test]
    fn missing_first_placement() {
        let out = sort(
            r#"[{"n":2},{"x":1},{"n":1}]"#,
            &opts("n", Order::Asc, Missing::First, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"x":1},{"n":1},{"n":2}]"#);
    }

    #[test]
    fn json_null_treated_as_missing() {
        // Explicit null sorts to the missing position (last here), not as a value.
        let out = sort(
            r#"[{"n":5},{"n":null},{"n":1}]"#,
            &opts("n", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":1},{"n":5},{"n":null}]"#);
    }

    #[test]
    fn missing_placement_absolute_under_desc() {
        // Missing stays last even when the key sorts descending.
        let out = sort(
            r#"[{"n":2},{"x":1},{"n":1}]"#,
            &opts("n", Order::Desc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":2},{"n":1},{"x":1}]"#);
    }

    #[test]
    fn case_sensitive_default_uppercase_first() {
        // Codepoint order: 'B' (0x42) sorts before 'a' (0x61).
        let out = sort(
            r#"[{"s":"a"},{"s":"B"}]"#,
            &opts("s", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"s":"B"},{"s":"a"}]"#);
    }

    #[test]
    fn case_insensitive_grouping() {
        let out = sort(
            r#"[{"s":"banana"},{"s":"Apple"},{"s":"cherry"}]"#,
            &opts("s", Order::Asc, Missing::Last, true, 0),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"s":"Apple"},{"s":"banana"},{"s":"cherry"}]"#
        );
    }

    #[test]
    fn stable_secondary_order_preserved() {
        // Equal on the key → original relative order kept (stable sort).
        let out = sort(
            r#"[{"k":1,"id":"a"},{"k":1,"id":"b"},{"k":1,"id":"c"}]"#,
            &opts("k", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"k":1,"id":"a"},{"k":1,"id":"b"},{"k":1,"id":"c"}]"#
        );
    }

    #[test]
    fn pretty_indent_two() {
        let out = sort(
            r#"[{"n":2},{"n":1}]"#,
            &opts("n", Order::Asc, Missing::Last, false, 2),
        )
        .unwrap();
        assert_eq!(out, "[\n  {\n    \"n\": 1\n  },\n  {\n    \"n\": 2\n  }\n]");
    }

    #[test]
    fn indent_clamped_to_eight() {
        let out = sort(
            r#"[{"n":1}]"#,
            &opts("n", Order::Asc, Missing::Last, false, 99),
        )
        .unwrap();
        assert_eq!(out, "[\n        {\n                \"n\": 1\n        }\n]");
    }

    #[test]
    fn object_keys_inside_elements_are_not_reordered() {
        // This tool sorts elements, never keys — "z" stays before "a".
        let out = sort(
            r#"[{"z":1,"a":2,"k":2},{"z":1,"a":2,"k":1}]"#,
            &opts("k", Order::Asc, Missing::Last, false, 0),
        )
        .unwrap();
        assert_eq!(out, r#"[{"z":1,"a":2,"k":1},{"z":1,"a":2,"k":2}]"#);
    }

    #[test]
    fn rejects_non_array_input() {
        let e = sort(r#"{"n":1}"#, &opts("n", Order::Asc, Missing::Last, false, 0)).unwrap_err();
        assert!(e.contains("must be a JSON array"));
    }

    #[test]
    fn rejects_empty_keys() {
        let e = sort(r#"[{"n":1}]"#, &opts("", Order::Asc, Missing::Last, false, 0)).unwrap_err();
        assert!(e.contains("no sort keys"));
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(sort("[{bad}]", &Options { keys: "n".into(), ..Default::default() }).is_err());
        assert!(sort("", &Options { keys: "n".into(), ..Default::default() }).is_err());
        assert!(sort("[1,2,]", &Options { keys: "n".into(), ..Default::default() }).is_err());
    }
}
