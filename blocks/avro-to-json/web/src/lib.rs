//! Browser-facing wasm-bindgen wrapper for /tools/avro-to-json/.
//! Compiled with wasm-pack for the standalone /tools/avro-to-json/ page.
use wasm_bindgen::prelude::*;

/// Decode an Avro Object Container File (OCF) into JSON.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the .avro file bytes as a base64 or hex string.
/// - `encoding`: `"auto"` (default) / `"base64"` / `"hex"` (blank → auto).
/// - `format`: `"records"` (default) / `"ndjson"` / `"full"` (blank → records).
///
/// Throws a JS error string on invalid arguments or undecodable input.
#[wasm_bindgen]
pub fn run(input: &str, encoding: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_avro_to_json_core::decode(input, encoding, format)
        .map_err(|e| JsValue::from_str(&e))
}
