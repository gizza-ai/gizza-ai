//! Browser-facing wasm-bindgen wrapper for /tools/strip-ansi-codes/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Remove ANSI escape codes from `text`.
///
/// The standalone tool page passes every field value as a string:
/// - `scope`: `"all"` (blank → all) strips every escape sequence; `"color"`
///   strips only SGR colour/style codes.
///
/// Throws a JS error string on an invalid `scope`.
#[wasm_bindgen]
pub fn run(text: &str, scope: &str) -> Result<String, JsValue> {
    gizza_ai_strip_ansi_codes_core::strip(text, scope).map_err(|e| JsValue::from_str(&e))
}
