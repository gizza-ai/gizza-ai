//! Browser-facing wasm-bindgen wrapper for /tools/protobuf-decode/.
//! Compiled with wasm-pack for the standalone /tools/protobuf-decode/ page.
use wasm_bindgen::prelude::*;

/// Decode raw protobuf wire bytes into a field/wire-type tree.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the wire bytes as a base64 or hex string.
/// - `encoding`: `"auto"` (default) / `"base64"` / `"hex"` (blank → auto).
/// - `format`: `"json"` (default) / `"text"` (blank → json).
///
/// Throws a JS error string on invalid arguments or undecodable input.
#[wasm_bindgen]
pub fn run(input: &str, encoding: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_protobuf_decode_core::decode(input, encoding, format)
        .map_err(|e| JsValue::from_str(&e))
}
