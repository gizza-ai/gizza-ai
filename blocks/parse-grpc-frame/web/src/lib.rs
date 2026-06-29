//! Browser-facing wasm-bindgen wrapper for /tools/parse-grpc-frame/.
//! Compiled with wasm-pack for the standalone /tools/parse-grpc-frame/ page.
use wasm_bindgen::prelude::*;

/// Parse a gRPC length-prefixed message stream into frames + decoded protobuf.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the gRPC frame bytes as a base64 or hex string.
/// - `encoding`: `"auto"` (default) / `"base64"` / `"hex"` (blank → auto).
/// - `format`: `"json"` (default) / `"text"` (blank → json).
///
/// Throws a JS error string on invalid arguments or undecodable input.
#[wasm_bindgen]
pub fn run(input: &str, encoding: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_parse_grpc_frame_core::parse(input, encoding, format)
        .map_err(|e| JsValue::from_str(&e))
}
