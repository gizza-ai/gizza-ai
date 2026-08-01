//! Browser-facing wasm-bindgen wrapper for /tools/dynamodb-json-converter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, direction: &str, pretty: &str) -> Result<String, JsValue> {
    let dir = gizza_ai_dynamodb_json_converter_core::Direction::parse(direction)
        .map_err(|e| JsValue::from_str(&e))?;
    let pretty = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "yes" | "on"
    );
    gizza_ai_dynamodb_json_converter_core::convert(input, dir, pretty)
        .map_err(|e| JsValue::from_str(&e))
}
