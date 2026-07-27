//! Browser-facing wasm-bindgen wrapper for /tools/msgpack-to-json/.
//! Compiled with wasm-pack for the standalone /tools/msgpack-to-json/ page.
//!
//! Field order MUST match meta.toml: input, input_format, indent,
//! binary_format. The page passes every field value as a string.
use wasm_bindgen::prelude::*;

/// Decode a MessagePack blob (hex or base64) into pretty-printed JSON.
///
/// - `input`: the MessagePack bytes as a hex or base64 string.
/// - `input_format`: `"auto"` (default), `"hex"`, or `"base64"` (blank → auto).
/// - `indent`: spaces per nesting level, 0-8 (blank → 2; 0 minifies).
/// - `binary_format`: `"base64"` (default) or `"hex"` for `bin`/`ext` payloads.
///
/// Throws a JS error string on invalid arguments or undecodable input.
#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    indent: &str,
    binary_format: &str,
) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    gizza_ai_msgpack_to_json_core::run(input, input_format, n, binary_format)
        .map_err(|e| JsValue::from_str(&e))
}
