//! dynamodb-json-converter core — convert between DynamoDB typed attribute JSON
//! (the `AttributeValue` wire format, e.g. `{"id":{"S":"1"},"age":{"N":"30"}}`)
//! and plain JSON (`{"id":"1","age":30}`), in both directions.
//!
//! Pure Rust (serde_json only) → runs on every backend, including the chat
//! Service Worker. No AWS SDK, no network: it only reshapes JSON, so you can
//! paste the result straight into the AWS CLI, a Lambda, an SDK call, or your
//! app code.
//!
//! Two directions plus auto-detect:
//! * **marshal** (`to-dynamodb`) — plain JSON → DynamoDB typed JSON. A top-level
//!   object becomes an item map (`{name: {S: …}}`); a scalar/array becomes a bare
//!   `AttributeValue`. Strings → `S`, numbers → `N` (kept as strings, exact
//!   spelling), booleans → `BOOL`, null → `NULL`, arrays → `L`, objects → `M`.
//! * **unmarshal** (`from-dynamodb`) — DynamoDB typed JSON → plain JSON. Handles
//!   every AttributeValue tag: `S`, `N`, `B`, `BOOL`, `NULL`, `M`, `L`, and the
//!   set types `SS`, `NS`, `BS`. Sets become plain arrays; `N`/`NS` become JSON
//!   numbers; `B`/`BS` (binary) pass through as their base64 strings.
//! * **auto** — inspect the input and pick a direction: input that already looks
//!   like DynamoDB typed JSON is unmarshalled, everything else is marshalled.

use serde_json::{json, Map, Value};

/// Conversion direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Detect the direction from the input shape.
    Auto,
    /// Plain JSON → DynamoDB typed JSON (marshal).
    ToDynamodb,
    /// DynamoDB typed JSON → plain JSON (unmarshal).
    FromDynamodb,
}

impl Direction {
    /// Parse the `direction` argument (`auto` | `to-dynamodb` | `from-dynamodb`).
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Direction::Auto),
            "to-dynamodb" | "to_dynamodb" | "marshal" => Ok(Direction::ToDynamodb),
            "from-dynamodb" | "from_dynamodb" | "unmarshal" => Ok(Direction::FromDynamodb),
            other => Err(format!(
                "invalid direction {other:?}: expected \"auto\", \"to-dynamodb\", or \"from-dynamodb\""
            )),
        }
    }
}

/// The ten DynamoDB `AttributeValue` type tags.
const TYPE_TAGS: [&str; 10] = ["S", "N", "B", "BOOL", "NULL", "M", "L", "SS", "NS", "BS"];

/// Convert `input` between plain JSON and DynamoDB typed JSON.
///
/// * `input` — the JSON document to convert.
/// * `direction` — `Auto` detects, `ToDynamodb` marshals, `FromDynamodb` unmarshals.
/// * `pretty` — pretty-print the output when true, compact single line when false.
pub fn convert(input: &str, direction: Direction, pretty: bool) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(input.trim())
        .map_err(|e| format!("input is not valid JSON: {e}"))?;

    let resolved = match direction {
        Direction::Auto => detect(&parsed),
        other => other,
    };

    let out = match resolved {
        Direction::ToDynamodb => marshal_top(&parsed),
        Direction::FromDynamodb => unmarshal_top(&parsed)?,
        Direction::Auto => unreachable!("auto is resolved before use"),
    };

    if pretty {
        serde_json::to_string_pretty(&out)
    } else {
        serde_json::to_string(&out)
    }
    .map_err(|e| format!("failed to serialize output: {e}"))
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Pick a direction for `Auto`: unmarshal input that already looks like DynamoDB
/// typed JSON (a bare AttributeValue, or a non-empty object whose values are all
/// AttributeValues), otherwise marshal.
fn detect(v: &Value) -> Direction {
    if is_attribute_value(v) {
        return Direction::FromDynamodb;
    }
    if let Value::Object(map) = v {
        if !map.is_empty() && map.values().all(is_attribute_value) {
            return Direction::FromDynamodb;
        }
    }
    Direction::ToDynamodb
}

/// True if `v` is a single-key object whose key is a DynamoDB type tag AND whose
/// inner value matches that tag's expected shape. The shape check keeps a plain
/// object like `{"N": 5}` (number inner, not the string `N` wants) or a field
/// literally named `S` from being mistaken for an AttributeValue.
fn is_attribute_value(v: &Value) -> bool {
    let Value::Object(map) = v else { return false };
    if map.len() != 1 {
        return false;
    }
    let (tag, inner) = map.iter().next().expect("len == 1");
    if !TYPE_TAGS.contains(&tag.as_str()) {
        return false;
    }
    match tag.as_str() {
        "S" | "B" => inner.is_string(),
        "N" => inner.as_str().is_some_and(is_number_literal),
        "BOOL" => inner.is_boolean(),
        "NULL" => inner.as_bool() == Some(true),
        "M" => inner.is_object(),
        "L" => inner.is_array(),
        "SS" | "BS" => inner.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        "NS" => inner
            .as_array()
            .is_some_and(|a| a.iter().all(|x| x.as_str().is_some_and(is_number_literal))),
        _ => false,
    }
}

/// True if `s` parses as a JSON number (DynamoDB stores `N` as a numeric string).
fn is_number_literal(s: &str) -> bool {
    serde_json::from_str::<Value>(s.trim()).is_ok_and(|v| v.is_number())
}

// ---------------------------------------------------------------------------
// Marshal: plain JSON -> DynamoDB typed JSON
// ---------------------------------------------------------------------------

/// Marshal the top-level document. An object becomes an item map (no outer `M`,
/// matching the DynamoDB item / PutItem shape); anything else becomes a bare
/// AttributeValue so scalars/arrays round-trip.
fn marshal_top(v: &Value) -> Value {
    match v {
        Value::Object(map) => Value::Object(marshal_item(map)),
        other => marshal_value(other),
    }
}

/// Marshal a map of field name → plain value into a map of field name → AttributeValue.
fn marshal_item(obj: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::with_capacity(obj.len());
    for (k, v) in obj {
        out.insert(k.clone(), marshal_value(v));
    }
    out
}

/// Marshal a single plain value into its DynamoDB AttributeValue wrapper.
fn marshal_value(v: &Value) -> Value {
    match v {
        Value::String(s) => json!({ "S": s }),
        // DynamoDB `N` is a string; keep the exact JSON number spelling.
        Value::Number(n) => json!({ "N": n.to_string() }),
        Value::Bool(b) => json!({ "BOOL": b }),
        Value::Null => json!({ "NULL": true }),
        Value::Array(arr) => json!({ "L": arr.iter().map(marshal_value).collect::<Vec<_>>() }),
        Value::Object(map) => json!({ "M": Value::Object(marshal_item(map)) }),
    }
}

// ---------------------------------------------------------------------------
// Unmarshal: DynamoDB typed JSON -> plain JSON
// ---------------------------------------------------------------------------

/// Unmarshal the top-level document. A bare AttributeValue is unwrapped to its
/// plain value; otherwise the input is treated as an item map and every field
/// value must be an AttributeValue.
fn unmarshal_top(v: &Value) -> Result<Value, String> {
    if is_attribute_value(v) {
        return unmarshal_value(v);
    }
    match v {
        Value::Object(map) => unmarshal_item(map).map(Value::Object),
        other => Err(format!(
            "DynamoDB JSON must be an object (an item map like {{\"id\":{{\"S\":\"1\"}}}} or a \
             single typed AttributeValue like {{\"S\":\"1\"}}), got {}",
            type_name(other)
        )),
    }
}

/// Unmarshal a map of field name → AttributeValue into plain field name → value.
fn unmarshal_item(obj: &Map<String, Value>) -> Result<Map<String, Value>, String> {
    let mut out = Map::with_capacity(obj.len());
    for (k, v) in obj {
        let plain = unmarshal_value(v).map_err(|e| format!("field {k:?}: {e}"))?;
        out.insert(k.clone(), plain);
    }
    Ok(out)
}

/// Unmarshal a single AttributeValue into its plain JSON value.
fn unmarshal_value(v: &Value) -> Result<Value, String> {
    let map = v.as_object().ok_or_else(|| {
        format!(
            "expected a typed AttributeValue object (e.g. {{\"S\":\"…\"}}), got {}",
            type_name(v)
        )
    })?;
    if map.len() != 1 {
        return Err(format!(
            "an AttributeValue must have exactly one type key, got {} ({})",
            map.len(),
            map.keys().map(String::as_str).collect::<Vec<_>>().join(", ")
        ));
    }
    let (tag, inner) = map.iter().next().expect("len == 1");
    match tag.as_str() {
        "S" | "B" => inner
            .as_str()
            .map(|s| Value::String(s.to_string()))
            .ok_or_else(|| format!("{tag} value must be a string, got {}", type_name(inner))),
        "N" => number_from_str(inner.as_str().ok_or_else(|| {
            format!("N value must be a numeric string, got {}", type_name(inner))
        })?),
        "BOOL" => inner
            .as_bool()
            .map(Value::Bool)
            .ok_or_else(|| format!("BOOL value must be true or false, got {}", type_name(inner))),
        "NULL" => {
            if inner.as_bool() == Some(true) {
                Ok(Value::Null)
            } else {
                Err("NULL value must be true".to_string())
            }
        }
        "M" => {
            let m = inner
                .as_object()
                .ok_or_else(|| format!("M value must be an object, got {}", type_name(inner)))?;
            unmarshal_item(m).map(Value::Object)
        }
        "L" => {
            let arr = inner
                .as_array()
                .ok_or_else(|| format!("L value must be an array, got {}", type_name(inner)))?;
            arr.iter()
                .map(unmarshal_value)
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        "SS" | "BS" => {
            let arr = inner.as_array().ok_or_else(|| {
                format!("{tag} value must be an array of strings, got {}", type_name(inner))
            })?;
            let mut out = Vec::with_capacity(arr.len());
            for (i, x) in arr.iter().enumerate() {
                let s = x
                    .as_str()
                    .ok_or_else(|| format!("{tag} element {i} must be a string, got {}", type_name(x)))?;
                out.push(Value::String(s.to_string()));
            }
            Ok(Value::Array(out))
        }
        "NS" => {
            let arr = inner.as_array().ok_or_else(|| {
                format!("NS value must be an array of numeric strings, got {}", type_name(inner))
            })?;
            let mut out = Vec::with_capacity(arr.len());
            for (i, x) in arr.iter().enumerate() {
                let s = x.as_str().ok_or_else(|| {
                    format!("NS element {i} must be a numeric string, got {}", type_name(x))
                })?;
                out.push(number_from_str(s).map_err(|e| format!("NS element {i}: {e}"))?);
            }
            Ok(Value::Array(out))
        }
        other => Err(format!(
            "unknown DynamoDB type tag {other:?}: expected one of {}",
            TYPE_TAGS.join(", ")
        )),
    }
}

/// Parse a DynamoDB `N` string into a plain JSON number, preserving exact
/// integer spelling up to the u64/i64 range (larger integers and
/// higher-precision decimals fall back to a 64-bit float — DynamoDB stores `N`
/// as strings precisely for this reason; see the page's limits note).
fn number_from_str(s: &str) -> Result<Value, String> {
    let t = s.trim();
    let v: Value = serde_json::from_str(t).map_err(|_| format!("invalid number {s:?}"))?;
    if v.is_number() {
        Ok(v)
    } else {
        Err(format!("invalid number {s:?}"))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).expect("valid JSON")
    }

    #[test]
    fn marshal_item_all_types() {
        let input = r#"{
            "id": "user#1",
            "age": 30,
            "score": 9.5,
            "big": 12345678901234567890,
            "active": true,
            "deleted": null,
            "tags": ["a", 2, false],
            "profile": { "name": "Ada", "level": 3 }
        }"#;
        let out = convert(input, Direction::ToDynamodb, false).expect("ok");
        let v = parse(&out);
        assert_eq!(v["id"], json!({ "S": "user#1" }));
        assert_eq!(v["age"], json!({ "N": "30" }));
        assert_eq!(v["score"], json!({ "N": "9.5" }));
        assert_eq!(v["big"], json!({ "N": "12345678901234567890" }));
        assert_eq!(v["active"], json!({ "BOOL": true }));
        assert_eq!(v["deleted"], json!({ "NULL": true }));
        assert_eq!(v["tags"], json!({ "L": [ { "S": "a" }, { "N": "2" }, { "BOOL": false } ] }));
        assert_eq!(v["profile"], json!({ "M": { "name": { "S": "Ada" }, "level": { "N": "3" } } }));
    }

    #[test]
    fn marshal_scalar_is_bare_attribute_value() {
        assert_eq!(
            parse(&convert(r#""hi""#, Direction::ToDynamodb, false).unwrap()),
            json!({ "S": "hi" })
        );
        assert_eq!(
            parse(&convert("[1,2]", Direction::ToDynamodb, false).unwrap()),
            json!({ "L": [ { "N": "1" }, { "N": "2" } ] })
        );
    }

    #[test]
    fn unmarshal_item_all_types() {
        let input = r#"{
            "id":   { "S": "user#1" },
            "age":  { "N": "30" },
            "score":{ "N": "9.5" },
            "big":  { "N": "12345678901234567890" },
            "active": { "BOOL": true },
            "deleted": { "NULL": true },
            "tags": { "L": [ { "S": "a" }, { "N": "2" }, { "BOOL": false } ] },
            "profile": { "M": { "name": { "S": "Ada" }, "level": { "N": "3" } } },
            "roles": { "SS": [ "admin", "user" ] },
            "scores": { "NS": [ "1", "2.5" ] }
        }"#;
        let out = convert(input, Direction::FromDynamodb, false).expect("ok");
        let v = parse(&out);
        assert_eq!(v["id"], json!("user#1"));
        assert_eq!(v["age"], json!(30));
        assert_eq!(v["score"], json!(9.5));
        assert_eq!(v["big"], json!(12345678901234567890u64));
        assert_eq!(v["active"], json!(true));
        assert_eq!(v["deleted"], Value::Null);
        assert_eq!(v["tags"], json!(["a", 2, false]));
        assert_eq!(v["profile"], json!({ "name": "Ada", "level": 3 }));
        assert_eq!(v["roles"], json!(["admin", "user"]));
        assert_eq!(v["scores"], json!([1, 2.5]));
    }

    #[test]
    fn unmarshal_binary_passes_base64_through() {
        let out = convert(
            r#"{ "blob": { "B": "ZGF0YQ==" }, "blobs": { "BS": [ "AAECAw==" ] } }"#,
            Direction::FromDynamodb,
            false,
        )
        .unwrap();
        let v = parse(&out);
        assert_eq!(v["blob"], json!("ZGF0YQ=="));
        assert_eq!(v["blobs"], json!(["AAECAw=="]));
    }

    #[test]
    fn unmarshal_bare_attribute_value() {
        assert_eq!(
            parse(&convert(r#"{ "S": "hi" }"#, Direction::FromDynamodb, false).unwrap()),
            json!("hi")
        );
        assert_eq!(
            parse(&convert(r#"{ "N": "42" }"#, Direction::FromDynamodb, false).unwrap()),
            json!(42)
        );
    }

    #[test]
    fn round_trip_marshal_then_unmarshal() {
        let plain = r#"{"id":"1","n":3,"nested":{"k":[true,null,"x"]}}"#;
        let dynamo = convert(plain, Direction::ToDynamodb, false).unwrap();
        let back = convert(&dynamo, Direction::FromDynamodb, false).unwrap();
        assert_eq!(parse(&back), parse(plain));
    }

    #[test]
    fn auto_detects_unmarshal() {
        let out = convert(r#"{ "id": { "S": "1" }, "n": { "N": "5" } }"#, Direction::Auto, false).unwrap();
        assert_eq!(parse(&out), json!({ "id": "1", "n": 5 }));
        assert_eq!(
            parse(&convert(r#"{ "BOOL": true }"#, Direction::Auto, false).unwrap()),
            json!(true)
        );
    }

    #[test]
    fn auto_detects_marshal() {
        let out = convert(r#"{ "id": "1", "n": 5 }"#, Direction::Auto, false).unwrap();
        assert_eq!(parse(&out), json!({ "id": { "S": "1" }, "n": { "N": "5" } }));
    }

    #[test]
    fn auto_field_named_like_a_tag_is_marshalled() {
        // {"N": 5} has a numeric (not string) inner, so it's a plain object -> marshal.
        let out = convert(r#"{ "N": 5 }"#, Direction::Auto, false).unwrap();
        assert_eq!(parse(&out), json!({ "N": { "N": "5" } }));
    }

    #[test]
    fn pretty_vs_compact() {
        assert!(convert(r#"{"a":1}"#, Direction::ToDynamodb, true).unwrap().contains('\n'));
        assert!(!convert(r#"{"a":1}"#, Direction::ToDynamodb, false).unwrap().contains('\n'));
    }

    #[test]
    fn invalid_json_errors() {
        assert!(convert("{ not json", Direction::Auto, true).is_err());
    }

    #[test]
    fn unmarshal_unknown_tag_errors() {
        let err = convert(r#"{ "id": { "XY": "1" } }"#, Direction::FromDynamodb, true).unwrap_err();
        assert!(err.contains("XY") || err.contains("one type key"), "got: {err}");
    }

    #[test]
    fn unmarshal_multi_key_attribute_value_errors() {
        let err = convert(r#"{ "id": { "S": "1", "N": "2" } }"#, Direction::FromDynamodb, true).unwrap_err();
        assert!(err.contains("exactly one type key"), "got: {err}");
    }

    #[test]
    fn unmarshal_bad_n_errors() {
        let err = convert(r#"{ "n": { "N": "abc" } }"#, Direction::FromDynamodb, true).unwrap_err();
        assert!(err.contains("invalid number"), "got: {err}");
    }

    #[test]
    fn unmarshal_non_object_errors() {
        let err = convert("[1,2,3]", Direction::FromDynamodb, true).unwrap_err();
        assert!(err.contains("must be an object"), "got: {err}");
    }

    #[test]
    fn direction_parse() {
        assert_eq!(Direction::parse("auto").unwrap(), Direction::Auto);
        assert_eq!(Direction::parse("to-dynamodb").unwrap(), Direction::ToDynamodb);
        assert_eq!(Direction::parse("from-dynamodb").unwrap(), Direction::FromDynamodb);
        assert!(Direction::parse("sideways").is_err());
    }
}
