//! Browser-facing wasm-bindgen wrapper for /tools/structured-data-validator/.
//! Compiled with wasm-pack for the standalone page. Field order MUST match
//! page/meta.toml: html, format.
use gizza_ai_structured_data_validator_core::{parse_format, validate};
use wasm_bindgen::prelude::*;

/// Validate the structured data in `html`.
///
/// The standalone tool page passes every field value as a string:
/// - `format`: `"report"` (blank → report) renders a human-readable report;
///   `"json"` returns a machine-readable `{counts, items, issues, valid}` summary.
///
/// Throws a JS error string on empty HTML or an invalid `format`.
#[wasm_bindgen]
pub fn run(html: &str, format: &str) -> Result<String, JsValue> {
    let fmt = parse_format(format).map_err(|e| JsValue::from_str(&e))?;
    validate(html, fmt).map_err(|e| JsValue::from_str(&e))
}
