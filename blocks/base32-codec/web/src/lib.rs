//! Browser-facing wasm-bindgen wrapper for /tools/base32-codec/.
//! Compiled with wasm-pack for the standalone /tools/base32-codec/ page.
use wasm_bindgen::prelude::*;

/// Encode data to Base32 or decode a Base32 string back.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as strings and are parsed here:
/// - `input`: the text/hex to encode, or the Base32 string to decode.
/// - `mode`: `"encode"`/`"decode"` (blank → encode).
/// - `variant`: `"standard"`/`"hex"`/`"crockford"`/`"zbase32"` (blank → standard).
/// - `format`: `"text"`/`"hex"` (blank → text).
/// - `lowercase`: `"true"`/`"1"`/`"yes"`/`"on"` → lowercase output; else off.
/// - `padding`: `"true"`/`"1"`/`"yes"`/`"on"` → padded (default); else unpadded.
///
/// Throws a JS error string on invalid arguments or an undecodable input.
#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    variant: &str,
    format: &str,
    lowercase: &str,
    padding: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on");
    let lowercase = truthy(lowercase);
    let padding = truthy(padding);
    gizza_ai_base32_codec_core::convert(input, mode, variant, format, lowercase, padding)
        .map_err(|e| JsValue::from_str(&e))
}
