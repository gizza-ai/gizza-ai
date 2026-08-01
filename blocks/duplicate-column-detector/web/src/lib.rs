//! Browser-facing wasm-bindgen wrapper for /tools/duplicate-column-detector/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    header: &str,
    delimiter: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    ignore_header_name: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_duplicate_column_detector_core::detect(
        data,
        truthy(header, true),
        delimiter,
        truthy(ignore_case, true),
        truthy(ignore_whitespace, true),
        truthy(ignore_header_name, true),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
