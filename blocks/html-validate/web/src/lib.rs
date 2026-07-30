//! Browser-facing wasm-bindgen wrapper for /tools/html-validate/.
//! Compiled with wasm-pack for the standalone page. Field order MUST match
//! page/meta.toml: html, format.
use wasm_bindgen::prelude::*;

/// Validate `html` and render the result.
///
/// The standalone tool page passes every field value as a string:
/// - `format`: `"report"` (blank → report) renders a human-readable issue list;
///   `"json"` returns a machine-readable `{valid, errors, warnings, elements, issues}` object.
///
/// Throws a JS error string on empty HTML or an invalid `format`.
#[wasm_bindgen]
pub fn run(html: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_html_validate_core::run(html, format).map_err(|e| JsValue::from_str(&e))
}
