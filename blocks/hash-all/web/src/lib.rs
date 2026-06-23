//! Browser-facing wasm-bindgen wrapper for /tools/hash-all/.
//! Compiled with wasm-pack for the standalone /tools/hash-all/ page.
use wasm_bindgen::prelude::*;

/// Compute EVERY common digest of `text` and return them as a labeled table.
///
/// The standalone tool page passes every field value as a string:
/// - `text`: the input to hash.
/// - `input_encoding`: `"text"` (blank → text) / `"hex"` / `"base64"`.
/// - `output_format`: `"hex"` (blank → hex) / `"base64"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase hex; anything else → off.
///
/// Throws a JS error string on an invalid encoding/format or undecodable input.
#[wasm_bindgen]
pub fn run(
    text: &str,
    input_encoding: &str,
    output_format: &str,
    uppercase: &str,
) -> Result<String, JsValue> {
    let uppercase = matches!(
        uppercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_hash_all_core::hash_all(text, input_encoding, output_format, uppercase)
        .map_err(|e| JsValue::from_str(&e))
}
