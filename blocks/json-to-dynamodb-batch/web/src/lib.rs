//! Browser-facing wasm-bindgen wrapper for /tools/json-to-dynamodb-batch/.
use wasm_bindgen::prelude::*;

fn truthy_default_on(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    table_name: &str,
    operation: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let operation = if operation.trim().is_empty() {
        "put"
    } else {
        operation.trim()
    };
    gizza_ai_json_to_dynamodb_batch_core::run(
        json,
        table_name,
        operation,
        truthy_default_on(pretty),
    )
    .map_err(|e| JsValue::from_str(&e))
}
