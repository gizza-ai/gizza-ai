//! Browser-facing wasm-bindgen wrapper for /tools/binary-codec/.
//! Compiled with wasm-pack for the standalone /tools/binary-codec/ page.
use wasm_bindgen::prelude::*;

/// Encode text to a binary bit string or decode a binary string back to text.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the text to encode, or the binary string to decode.
/// - `mode`: `"encode"`/`"decode"` (blank → encode).
/// - `format`: `"text"`/`"bytes"` (blank → text) — how to render decoded bytes.
/// - `delimiter`: `"none"`/`"space"`/`"colon"`/`"dash"`/`"comma"`/`"newline"`.
/// - `prefix`: `"none"`/`"0b"`.
///
/// Throws a JS error string on invalid arguments or an undecodable input.
#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    format: &str,
    delimiter: &str,
    prefix: &str,
) -> Result<String, JsValue> {
    gizza_ai_binary_codec_core::convert(input, mode, format, delimiter, prefix)
        .map_err(|e| JsValue::from_str(&e))
}
