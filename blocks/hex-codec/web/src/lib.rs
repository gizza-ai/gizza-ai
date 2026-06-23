//! Browser-facing wasm-bindgen wrapper for /tools/hex-codec/.
//! Compiled with wasm-pack for the standalone /tools/hex-codec/ page.
use wasm_bindgen::prelude::*;

/// Encode text to a hex string or decode a hex string back to text.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean param arrives as a string and is parsed here:
/// - `input`: the text to encode, or the hex string to decode.
/// - `mode`: `"encode"`/`"decode"` (blank → encode).
/// - `format`: `"text"`/`"bytes"` (blank → text) — how to render decoded bytes.
/// - `delimiter`: `"none"`/`"space"`/`"colon"`/`"dash"`/`"comma"`/`"newline"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase digits; else lowercase.
/// - `prefix`: `"none"`/`"0x"`/`"\x"`.
///
/// Throws a JS error string on invalid arguments or an undecodable input.
#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    format: &str,
    delimiter: &str,
    uppercase: &str,
    prefix: &str,
) -> Result<String, JsValue> {
    let truthy =
        matches!(uppercase.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on");
    gizza_ai_hex_codec_core::convert(input, mode, format, delimiter, truthy, prefix)
        .map_err(|e| JsValue::from_str(&e))
}
