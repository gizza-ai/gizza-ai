//! Browser-facing wasm-bindgen wrapper for /tools/rtf-to-text/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Convert RTF markup to plain text.
///
/// The standalone tool page passes every field value as a string:
/// - `rtf`: the raw RTF document source.
/// - `line_breaks`: `"preserve"` (blank → preserve) keeps paragraph newlines;
///   `"collapse"` flattens all whitespace to single spaces.
///
/// Throws a JS error string on non-RTF input or an invalid `line_breaks`.
#[wasm_bindgen]
pub fn run(rtf: &str, line_breaks: &str) -> Result<String, JsValue> {
    gizza_ai_rtf_to_text_core::rtf_to_text(rtf, line_breaks).map_err(|e| JsValue::from_str(&e))
}
