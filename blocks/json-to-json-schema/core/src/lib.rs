//! gizza-ai/json-to-json-schema core — infer a JSON Schema from one or more JSON
//! examples. No wafer/wasm-bindgen deps (serde_json only). When the sample root is
//! an array, its elements are merged so keys missing in some become optional and
//! differing types become unions; the same merge applies to any nested array.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

/// Which JSON Schema dialect to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Draft {
    /// Draft-07 (`http://json-schema.org/draft-07/schema#`).
    Draft07,
    /// Draft 2020-12 (`https://json-schema.org/draft/2020-12/schema`).
    Draft2020,
}

impl Draft {
    fn schema_uri(self) -> &'static str {
        match self {
            Draft::Draft07 => "http://json-schema.org/draft-07/schema#",
            Draft::Draft2020 => "https://json-schema.org/draft/2020-12/schema",
        }
    }
}

/// Inference options.
#[derive(Debug, Clone)]
pub struct Options {
    pub draft: Draft,
    /// Emit `"additionalProperties": false` on objects when `false` (strict).
    pub additional_properties: bool,
    /// List every key seen in all merged samples of an object under `required`.
    pub required: bool,
    /// Detect string `format` (email, uri, date-time, date, uuid, ipv4).
    pub detect_formats: bool,
    /// Root schema `title` (skipped when empty).
    pub title: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            draft: Draft::Draft2020,
            additional_properties: false,
            required: true,
            detect_formats: true,
            title: String::new(),
        }
    }
}

/// A structurally-merged type node.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    /// Set of JSON Schema primitive type names + an optional detected string format.
    Prim { types: BTreeSet<&'static str>, format: Option<&'static str> },
    Arr(Box<Node>),
    Obj(BTreeMap<String, Field>),
    /// No information yet (empty array element) — schema `{}`.
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    node: Node,
    required: bool,
}

fn prim(name: &'static str, format: Option<&'static str>) -> Node {
    let mut types = BTreeSet::new();
    types.insert(name);
    Node::Prim { types, format }
}

/// Build a Node from one JSON value.
fn from_value(v: &Value) -> Node {
    match v {
        Value::Null => prim("null", None),
        Value::Bool(_) => prim("boolean", None),
        Value::Number(n) => {
            // A number literal without a fraction/exponent parses as i64/u64 → integer.
            if n.is_i64() || n.is_u64() {
                prim("integer", None)
            } else {
                prim("number", None)
            }
        }
        Value::String(s) => prim("string", detect_format(s)),
        Value::Array(items) => {
            let mut inner = Node::Any;
            for it in items {
                inner = merge(inner, from_value(it));
            }
            Node::Arr(Box::new(inner))
        }
        Value::Object(map) => {
            let mut fields = BTreeMap::new();
            for (k, val) in map {
                fields.insert(k.clone(), Field { node: from_value(val), required: true });
            }
            Node::Obj(fields)
        }
    }
}

/// Unify two nodes.
fn merge(a: Node, b: Node) -> Node {
    match (a, b) {
        (Node::Any, x) | (x, Node::Any) => x,
        (
            Node::Prim { types: t1, format: f1 },
            Node::Prim { types: t2, format: f2 },
        ) => {
            // Keep a format only when both agree, or the other side is purely `null`.
            let format = if f1 == f2 {
                f1
            } else if t1.len() == 1 && t1.contains("null") {
                f2
            } else if t2.len() == 1 && t2.contains("null") {
                f1
            } else {
                None
            };
            let mut types = t1;
            types.extend(t2);
            // `number` already subsumes `integer`.
            if types.contains("number") {
                types.remove("integer");
            }
            Node::Prim { types, format }
        }
        (Node::Arr(a), Node::Arr(b)) => Node::Arr(Box::new(merge(*a, *b))),
        (Node::Obj(m1), Node::Obj(m2)) => {
            let mut out: BTreeMap<String, Field> = BTreeMap::new();
            let keys: BTreeSet<&String> = m1.keys().chain(m2.keys()).collect();
            for k in keys {
                match (m1.get(k), m2.get(k)) {
                    (Some(f1), Some(f2)) => out.insert(
                        k.clone(),
                        Field {
                            node: merge(f1.node.clone(), f2.node.clone()),
                            required: f1.required && f2.required,
                        },
                    ),
                    (Some(f), None) | (None, Some(f)) => {
                        out.insert(k.clone(), Field { node: f.node.clone(), required: false })
                    }
                    (None, None) => unreachable!(),
                };
            }
            Node::Obj(out)
        }
        // Incompatible shapes (e.g. object vs string) → no constraint.
        _ => Node::Any,
    }
}

/// Detect a JSON Schema `format` for a string sample. Order matters: more specific first.
fn detect_format(s: &str) -> Option<&'static str> {
    if is_uuid(s) {
        Some("uuid")
    } else if is_date_time(s) {
        Some("date-time")
    } else if is_date(s) {
        Some("date")
    } else if is_email(s) {
        Some("email")
    } else if is_ipv4(s) {
        Some("ipv4")
    } else if is_uri(s) {
        Some("uri")
    } else {
        None
    }
}

fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    for (i, c) in b.iter().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            if *c != b'-' {
                return false;
            }
        } else if !c.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn is_date(s: &str) -> bool {
    // YYYY-MM-DD
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9];
    if !digits.iter().all(|&i| b[i].is_ascii_digit()) {
        return false;
    }
    let month: u32 = s[5..7].parse().unwrap_or(0);
    let day: u32 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn is_date_time(s: &str) -> bool {
    // RFC 3339: date, a 'T'/'t'/' ' separator, then HH:MM:SS, then Z / ±HH:MM (fraction optional).
    if s.len() < 20 {
        return false;
    }
    let sep = s.as_bytes()[10];
    if !matches!(sep, b'T' | b't' | b' ') || !is_date(&s[..10]) {
        return false;
    }
    let rest = &s[11..];
    let time_end = rest.find(['Z', 'z', '+']).or_else(|| {
        // A '-' in the offset can't be one of the two time separators before it.
        rest.match_indices('-').map(|(i, _)| i).find(|&i| i >= 8)
    });
    let (time, zone) = match time_end {
        Some(i) => (&rest[..i], &rest[i..]),
        None => return false,
    };
    // time = HH:MM:SS(.fff)?
    let base = &time[..time.len().min(8)];
    let tb = base.as_bytes();
    if base.len() != 8 || tb[2] != b':' || tb[5] != b':' {
        return false;
    }
    if ![0, 1, 3, 4, 6, 7].iter().all(|&i| tb[i].is_ascii_digit()) {
        return false;
    }
    // zone = Z | z | ±HH:MM
    matches!(zone, "Z" | "z")
        || (zone.len() == 6
            && matches!(zone.as_bytes()[0], b'+' | b'-')
            && zone.as_bytes()[3] == b':'
            && [1, 2, 4, 5].iter().all(|&i| zone.as_bytes()[i].is_ascii_digit()))
}

fn is_email(s: &str) -> bool {
    let mut parts = s.split('@');
    let (local, domain) = match (parts.next(), parts.next(), parts.next()) {
        (Some(l), Some(d), None) => (l, d),
        _ => return false,
    };
    if local.is_empty() || domain.len() < 3 || !domain.contains('.') {
        return false;
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    let ok = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let ok_domain = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '-';
    local.chars().all(ok) && domain.chars().all(ok_domain)
}

fn is_ipv4(s: &str) -> bool {
    let octets: Vec<&str> = s.split('.').collect();
    octets.len() == 4
        && octets.iter().all(|o| {
            !o.is_empty()
                && o.len() <= 3
                && o.bytes().all(|c| c.is_ascii_digit())
                && o.parse::<u32>().map(|n| n <= 255).unwrap_or(false)
        })
}

fn is_uri(s: &str) -> bool {
    // scheme "://" rest, scheme starting with a letter.
    if let Some(idx) = s.find("://") {
        let scheme = &s[..idx];
        !scheme.is_empty()
            && scheme.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
            && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            && idx + 3 < s.len()
            && !s.contains(char::is_whitespace)
    } else {
        false
    }
}

/// Render a Node to a JSON Schema `Value`.
fn to_schema(node: &Node, opts: &Options) -> Value {
    match node {
        Node::Any => Value::Object(Map::new()),
        Node::Prim { types, format } => {
            let mut obj = Map::new();
            let type_val = if types.len() == 1 {
                Value::String(types.iter().next().unwrap().to_string())
            } else {
                Value::Array(types.iter().map(|t| Value::String(t.to_string())).collect())
            };
            obj.insert("type".into(), type_val);
            if opts.detect_formats {
                if let Some(f) = format {
                    // `uuid` is a format only from Draft 2019-09 onward.
                    if !(*f == "uuid" && opts.draft == Draft::Draft07) {
                        obj.insert("format".into(), Value::String(f.to_string()));
                    }
                }
            }
            Value::Object(obj)
        }
        Node::Arr(inner) => {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("array".into()));
            obj.insert("items".into(), to_schema(inner, opts));
            Value::Object(obj)
        }
        Node::Obj(fields) => {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("object".into()));
            let mut props = Map::new();
            let mut required: Vec<Value> = Vec::new();
            for (k, f) in fields {
                props.insert(k.clone(), to_schema(&f.node, opts));
                if opts.required && f.required {
                    required.push(Value::String(k.clone()));
                }
            }
            obj.insert("properties".into(), Value::Object(props));
            if opts.required && !required.is_empty() {
                obj.insert("required".into(), Value::Array(required));
            }
            if !opts.additional_properties {
                obj.insert("additionalProperties".into(), Value::Bool(false));
            }
            Value::Object(obj)
        }
    }
}

/// Infer a JSON Schema (pretty-printed) from a JSON sample string.
pub fn infer(json: &str, opts: &Options) -> Result<String, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let node = from_value(&value);
    let body = to_schema(&node, opts);

    // Build the root with $schema (and title) first so the output leads with them
    // even when serde_json preserves insertion order.
    let mut root = Map::new();
    root.insert("$schema".into(), Value::String(opts.draft.schema_uri().to_string()));
    if !opts.title.trim().is_empty() {
        root.insert("title".into(), Value::String(opts.title.trim().to_string()));
    }
    if let Value::Object(map) = body {
        root.extend(map);
    }

    serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| format!("could not serialize schema: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn simple_object() {
        let s = infer(r#"{"name":"Ada","age":30,"active":true}"#, &opts()).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "object");
        assert_eq!(v["properties"]["name"]["type"], "string");
        assert_eq!(v["properties"]["age"]["type"], "integer");
        assert_eq!(v["properties"]["active"]["type"], "boolean");
        let req = v["required"].as_array().unwrap();
        assert!(req.contains(&Value::String("name".into())));
        assert_eq!(v["additionalProperties"], Value::Bool(false));
        assert_eq!(v["$schema"], "https://json-schema.org/draft/2020-12/schema");
    }

    #[test]
    fn array_merges_optional_and_union() {
        let s = infer(r#"[{"a":1},{"a":2,"b":"x"}]"#, &opts()).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "array");
        let item = &v["items"];
        assert_eq!(item["properties"]["a"]["type"], "integer");
        // `b` present in only one element → not required.
        let req = item["required"].as_array().unwrap();
        assert!(req.contains(&Value::String("a".into())));
        assert!(!req.contains(&Value::String("b".into())));
    }

    #[test]
    fn mixed_primitive_types_become_union() {
        let s = infer(r#"{"v":[1,"two",true]}"#, &opts()).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        let types = v["properties"]["v"]["items"]["type"].as_array().unwrap();
        assert!(types.contains(&Value::String("boolean".into())));
        assert!(types.contains(&Value::String("integer".into())));
        assert!(types.contains(&Value::String("string".into())));
    }

    #[test]
    fn integer_and_float_collapse_to_number() {
        let s = infer(r#"[1, 2.5]"#, &opts()).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["items"]["type"], "number");
    }

    #[test]
    fn detects_formats() {
        let json = r#"{"e":"ada@example.com","u":"https://x.io/a","d":"2020-01-02","dt":"2020-01-02T03:04:05Z","id":"12345678-1234-1234-1234-1234567890ab","ip":"10.0.0.1"}"#;
        let s = infer(json, &opts()).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        let p = &v["properties"];
        assert_eq!(p["e"]["format"], "email");
        assert_eq!(p["u"]["format"], "uri");
        assert_eq!(p["d"]["format"], "date");
        assert_eq!(p["dt"]["format"], "date-time");
        assert_eq!(p["id"]["format"], "uuid");
        assert_eq!(p["ip"]["format"], "ipv4");
    }

    #[test]
    fn format_detection_can_be_disabled() {
        let mut o = opts();
        o.detect_formats = false;
        let s = infer(r#"{"e":"ada@example.com"}"#, &o).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v["properties"]["e"].get("format").is_none());
    }

    #[test]
    fn uuid_format_omitted_for_draft07() {
        let mut o = opts();
        o.draft = Draft::Draft07;
        let s = infer(r#"{"id":"12345678-1234-1234-1234-1234567890ab"}"#, &o).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["properties"]["id"]["type"], "string");
        assert!(v["properties"]["id"].get("format").is_none());
        assert_eq!(v["$schema"], "http://json-schema.org/draft-07/schema#");
    }

    #[test]
    fn additional_properties_allowed_when_true() {
        let mut o = opts();
        o.additional_properties = true;
        let s = infer(r#"{"a":1}"#, &o).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("additionalProperties").is_none());
    }

    #[test]
    fn required_can_be_disabled() {
        let mut o = opts();
        o.required = false;
        let s = infer(r#"{"a":1}"#, &o).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("required").is_none());
    }

    #[test]
    fn title_is_added() {
        let mut o = opts();
        o.title = "User".into();
        let s = infer(r#"{"a":1}"#, &o).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["title"], "User");
    }

    #[test]
    fn nullable_field_is_union_with_null() {
        let s = infer(r#"[{"x":1},{"x":null}]"#, &opts()).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        let types = v["items"]["properties"]["x"]["type"].as_array().unwrap();
        assert!(types.contains(&Value::String("integer".into())));
        assert!(types.contains(&Value::String("null".into())));
    }

    #[test]
    fn invalid_json_errors() {
        assert!(infer("{not json}", &opts()).is_err());
    }

    #[test]
    fn non_format_string_has_no_format() {
        let s = infer(r#"{"s":"just words"}"#, &opts()).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v["properties"]["s"].get("format").is_none());
    }
}
