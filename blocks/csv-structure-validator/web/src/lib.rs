//! Browser-facing wasm-bindgen wrapper for /tools/csv-structure-validator/.
//! Field order MUST match meta.toml: data, header, delimiter, quote, comment,
//! max_issues.
use gizza_ai_csv_structure_validator_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    header: &str,
    delimiter: &str,
    quote: &str,
    comment: &str,
    max_issues: &str,
) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let max = if max_issues.trim().is_empty() {
        50usize
    } else {
        max_issues
            .trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str("max_issues must be a whole number between 1 and 1000"))?
    };
    summary(data, hdr, delimiter, quote, comment, max).map_err(|e| JsValue::from_str(&e))
}
