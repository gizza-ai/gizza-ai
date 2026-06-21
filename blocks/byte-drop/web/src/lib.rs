//! Browser-facing wasm-bindgen wrapper for /tools/byte-drop/.
//! Field order MUST match meta.toml: input, start, length, format, output.
//! The standalone tool page passes every field value as a string, so the
//! integer params (`start`, `length`) are parsed here.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    start: &str,
    length: &str,
    format: &str,
    output: &str,
) -> Result<String, JsValue> {
    let start_i: i64 = start.trim().parse().unwrap_or(0);
    let length_i: i64 = length.trim().parse().unwrap_or(0);
    gizza_ai_byte_drop_core::drop_bytes(input, start_i, length_i, format, output)
        .map_err(|e| JsValue::from_str(&e))
}
