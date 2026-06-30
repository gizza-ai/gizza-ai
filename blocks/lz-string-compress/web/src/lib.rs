//! Browser-facing wasm-bindgen wrapper for /tools/lz-string-compress/.
//! Compiled with wasm-pack for the standalone /tools/lz-string-compress/ page.
use wasm_bindgen::prelude::*;

/// Compress or decompress `text` with LZ-String.
///
/// The standalone tool page passes every field value as a string:
/// - `mode`: `"compress"` (blank → compress) / `"decompress"`.
/// - `format`: `"base64"` (blank → base64) / `"uri"` / `"utf16"`.
///
/// Throws a JS error string on an invalid `mode`/`format` or an undecodable
/// decompress input.
#[wasm_bindgen]
pub fn run(text: &str, mode: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_lz_string_compress_core::convert(text, mode, format)
        .map_err(|e| JsValue::from_str(&e))
}
