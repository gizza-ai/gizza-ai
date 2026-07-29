//! Browser-facing wasm-bindgen wrapper for /tools/base64url-converter/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Transcode `text` between standard Base64 and URL-safe Base64url.
///
/// The tool page passes every field value as a string, so the boolean arrives
/// as a string and is parsed here:
/// - `direction`: `"auto"`/`"to-url"`/`"to-standard"` (blank → auto).
/// - `padding`: `"auto"`/`"keep"`/`"strip"` (blank → auto).
/// - `validate`: `"true"`/`"1"`/`"yes"`/`"on"` → validate; anything else → off.
///
/// Throws a JS error string on an invalid `direction`/`padding`, a stray
/// character, or (when validating) input that is not decodable Base64.
#[wasm_bindgen]
pub fn run(text: &str, direction: &str, padding: &str, validate: &str) -> Result<String, JsValue> {
    let validate = matches!(
        validate.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_base64url_converter_core::convert(text, direction, padding, validate)
        .map_err(|e| JsValue::from_str(&e))
}
