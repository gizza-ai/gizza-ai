//! Browser-facing wasm-bindgen wrapper for /tools/base58-codec/.
//! Compiled with wasm-pack for the standalone /tools/base58-codec/ page.
use wasm_bindgen::prelude::*;

/// Encode data to Base58 or decode a Base58 string back.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the text/hex to encode, or the Base58 string to decode.
/// - `mode`: `"encode"`/`"decode"` (blank → encode).
/// - `variant`: `"bitcoin"`/`"ripple"`/`"flickr"` (blank → bitcoin).
/// - `format`: `"text"`/`"hex"` (blank → text).
///
/// Throws a JS error string on invalid arguments or an undecodable input.
#[wasm_bindgen]
pub fn run(input: &str, mode: &str, variant: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_base58_codec_core::convert(input, mode, variant, format)
        .map_err(|e| JsValue::from_str(&e))
}
