//! Browser-facing wasm-bindgen wrapper for /tools/diff-extract-versions/.
//! The page marshals every field as a string, so the checkbox string is parsed
//! here and blank enum fields fall back to their descriptor defaults before the
//! pure core is called.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    diff: &str,
    output: &str,
    file: &str,
    gaps: &str,
    line_numbers: &str,
) -> Result<String, JsValue> {
    gizza_ai_diff_extract_versions_core::extract_versions(
        diff,
        output,
        file,
        gaps,
        truthy(line_numbers),
    )
    .map_err(|e| JsValue::from_str(&e))
}
