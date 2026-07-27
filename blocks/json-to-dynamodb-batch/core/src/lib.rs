//! json-to-dynamodb-batch core — turn a JSON array of objects into a DynamoDB
//! `BatchWriteItem` request payload (PutRequest or DeleteRequest entries).
//!
//! Pure Rust (serde_json only) → runs on every backend, including the chat
//! Service Worker. No AWS SDK, no network: it only shapes JSON into the typed
//! AttributeValue map that the DynamoDB `BatchWriteItem` API expects, so you can
//! paste the result straight into the AWS CLI, a Lambda, or an SDK call.
//!
//! DynamoDB caps a single `BatchWriteItem` call at 25 write requests, so this
//! block errors above 25 items rather than emitting a payload AWS would reject.

use serde_json::{json, Map, Value};

/// Maximum write requests allowed in a single DynamoDB BatchWriteItem call.
pub const MAX_ITEMS: usize = 25;

/// Build a DynamoDB `BatchWriteItem` payload from a JSON array of objects.
///
/// * `json_src` — a JSON array of objects. Each object becomes one write request.
/// * `table_name` — the target DynamoDB table (must be non-empty).
/// * `operation` — `put` (default) emits `PutRequest`/`Item`; `delete` emits
///   `DeleteRequest`/`Key`. For `delete`, each object should hold only the key
///   attributes.
/// * `pretty` — pretty-print the JSON output when true, compact when false.
///
/// Returns the serialized `{ "RequestItems": { table: [ ... ] } }` payload.
pub fn run(
    json_src: &str,
    table_name: &str,
    operation: &str,
    pretty: bool,
) -> Result<String, String> {
    let table = table_name.trim();
    if table.is_empty() {
        return Err("table_name must not be empty".into());
    }

    let operation = operation.trim().to_ascii_lowercase();
    let (wrapper, field) = match operation.as_str() {
        "put" => ("PutRequest", "Item"),
        "delete" => ("DeleteRequest", "Key"),
        other => {
            return Err(format!(
                "invalid operation {other:?}: expected \"put\" or \"delete\""
            ))
        }
    };

    let parsed: Value =
        serde_json::from_str(json_src.trim()).map_err(|e| format!("input is not valid JSON: {e}"))?;
    let items = parsed
        .as_array()
        .ok_or_else(|| "input must be a JSON array of objects (e.g. [ { ... } ])".to_string())?;

    if items.len() > MAX_ITEMS {
        return Err(format!(
            "{} items exceeds the DynamoDB BatchWriteItem limit of {MAX_ITEMS} per call",
            items.len()
        ));
    }

    let mut requests: Vec<Value> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| {
            format!("array item {i} must be a JSON object (got {})", type_name(item))
        })?;
        let typed = to_attribute_map(obj);
        requests.push(json!({ wrapper: { field: typed } }));
    }

    let payload = json!({ "RequestItems": { table: requests } });

    if pretty {
        serde_json::to_string_pretty(&payload)
    } else {
        serde_json::to_string(&payload)
    }
    .map_err(|e| format!("failed to serialize output: {e}"))
}

/// Convert a JSON object into a DynamoDB typed AttributeValue map.
fn to_attribute_map(obj: &Map<String, Value>) -> Value {
    let mut out = Map::with_capacity(obj.len());
    for (k, v) in obj {
        out.insert(k.clone(), to_attribute_value(v));
    }
    Value::Object(out)
}

/// Convert a single JSON value into its DynamoDB AttributeValue wrapper.
fn to_attribute_value(v: &Value) -> Value {
    match v {
        Value::String(s) => json!({ "S": s }),
        // Preserve the exact JSON number spelling — DynamoDB `N` is a string.
        Value::Number(n) => json!({ "N": n.to_string() }),
        Value::Bool(b) => json!({ "BOOL": b }),
        Value::Null => json!({ "NULL": true }),
        Value::Array(arr) => {
            let list: Vec<Value> = arr.iter().map(to_attribute_value).collect();
            json!({ "L": list })
        }
        Value::Object(map) => json!({ "M": to_attribute_map(map) }),
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
        serde_json::from_str(s).expect("valid JSON output")
    }

    #[test]
    fn put_happy_path_with_all_types() {
        let input = r#"[
            {
                "id": "user#1",
                "age": 30,
                "score": 9.5,
                "big": 12345678901234567890,
                "active": true,
                "deleted": null,
                "tags": ["a", 2, false],
                "profile": { "name": "Ada", "level": 3 }
            }
        ]"#;
        let out = run(input, "Users", "put", true).expect("ok");
        let v = parse(&out);
        let item = &v["RequestItems"]["Users"][0]["PutRequest"]["Item"];

        assert_eq!(item["id"], json!({ "S": "user#1" }));
        assert_eq!(item["age"], json!({ "N": "30" }));
        assert_eq!(item["score"], json!({ "N": "9.5" }));
        // Big integer preserved exactly via serde_json::Number::to_string.
        assert_eq!(item["big"], json!({ "N": "12345678901234567890" }));
        assert_eq!(item["active"], json!({ "BOOL": true }));
        assert_eq!(item["deleted"], json!({ "NULL": true }));
        assert_eq!(
            item["tags"],
            json!({ "L": [ { "S": "a" }, { "N": "2" }, { "BOOL": false } ] })
        );
        assert_eq!(
            item["profile"],
            json!({ "M": { "name": { "S": "Ada" }, "level": { "N": "3" } } })
        );
    }

    #[test]
    fn put_output_is_pretty_by_default() {
        let out = run(r#"[{"id":"1"}]"#, "T", "put", true).unwrap();
        assert!(out.contains('\n'), "pretty output should have newlines");
    }

    #[test]
    fn delete_emits_key_and_compact() {
        let input = r#"[{ "id": "user#1", "sort": 5 }]"#;
        let out = run(input, "Users", "delete", false).expect("ok");
        assert!(!out.contains('\n'), "compact output should be single line");
        let v = parse(&out);
        let key = &v["RequestItems"]["Users"][0]["DeleteRequest"]["Key"];
        assert_eq!(key["id"], json!({ "S": "user#1" }));
        assert_eq!(key["sort"], json!({ "N": "5" }));
        // Delete requests must not carry a PutRequest.
        assert!(v["RequestItems"]["Users"][0].get("PutRequest").is_none());
    }

    #[test]
    fn invalid_json_errors() {
        assert!(run("{ not json", "T", "put", true).is_err());
    }

    #[test]
    fn non_array_errors() {
        let err = run(r#"{"id":"1"}"#, "T", "put", true).unwrap_err();
        assert!(err.contains("array"), "got: {err}");
    }

    #[test]
    fn non_object_item_errors() {
        let err = run(r#"[{"id":"1"}, 42]"#, "T", "put", true).unwrap_err();
        assert!(err.contains("object"), "got: {err}");
    }

    #[test]
    fn over_25_items_errors() {
        let mut rows = Vec::new();
        for i in 0..26 {
            rows.push(format!(r#"{{"id":"{i}"}}"#));
        }
        let input = format!("[{}]", rows.join(","));
        let err = run(&input, "T", "put", false).unwrap_err();
        assert!(err.contains("25"), "got: {err}");
    }

    #[test]
    fn exactly_25_items_ok() {
        let mut rows = Vec::new();
        for i in 0..25 {
            rows.push(format!(r#"{{"id":"{i}"}}"#));
        }
        let input = format!("[{}]", rows.join(","));
        let out = run(&input, "T", "put", false).expect("25 is allowed");
        let v = parse(&out);
        assert_eq!(v["RequestItems"]["T"].as_array().unwrap().len(), 25);
    }

    #[test]
    fn empty_table_name_errors() {
        assert!(run(r#"[{"id":"1"}]"#, "   ", "put", true).is_err());
    }

    #[test]
    fn invalid_operation_errors() {
        assert!(run(r#"[{"id":"1"}]"#, "T", "upsert", true).is_err());
    }
}
