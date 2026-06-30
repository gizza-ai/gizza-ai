//! Browser-facing wasm-bindgen wrapper for /tools/parse-websocket-frame/.
//! Compiled with wasm-pack for the standalone /tools/parse-websocket-frame/ page.
use wasm_bindgen::prelude::*;

/// Decode a WebSocket frame (RFC 6455) into its header fields + unmasked payload.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the WebSocket frame bytes as a base64 or hex string.
/// - `encoding`: `"auto"` (default) / `"base64"` / `"hex"` (blank → auto).
/// - `format`: `"json"` (default) / `"text"` (blank → json).
///
/// Throws a JS error string on invalid arguments or undecodable input.
#[wasm_bindgen]
pub fn run(input: &str, encoding: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_parse_websocket_frame_core::parse(input, encoding, format)
        .map_err(|e| JsValue::from_str(&e))
}
