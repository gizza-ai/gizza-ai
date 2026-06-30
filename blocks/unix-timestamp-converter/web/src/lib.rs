//! Browser-facing wasm-bindgen wrapper for /tools/unix-timestamp-converter/.
//! The standalone page passes every field value as a string.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(value: &str, mode: &str, unit: &str) -> Result<String, JsValue> {
    gizza_ai_unix_timestamp_converter_core::run(value, mode, unit)
        .map_err(|e| JsValue::from_str(&e))
}
