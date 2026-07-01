//! Browser-facing wasm-bindgen wrapper for /tools/base62-codec/.
//! Compiled with wasm-pack for the standalone /tools/base62-codec/ page.
use wasm_bindgen::prelude::*;

/// Encode data to Base62 or decode a Base62 string back.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the text/hex/number to encode, or the Base62 string to decode.
/// - `mode`: `"encode"`/`"decode"` (blank → encode).
/// - `variant`: `"standard"`/`"inverted"` (blank → standard).
/// - `format`: `"text"`/`"hex"`/`"number"` (blank → text).
///
/// Throws a JS error string on invalid arguments or an undecodable input.
#[wasm_bindgen]
pub fn run(input: &str, mode: &str, variant: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_base62_codec_core::convert(input, mode, variant, format)
        .map_err(|e| JsValue::from_str(&e))
}
