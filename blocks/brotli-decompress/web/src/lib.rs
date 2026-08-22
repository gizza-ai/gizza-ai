//! Browser-facing wasm-bindgen wrapper for /tools/brotli-decompress/.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as the strings "true"/"false" — match positively.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(data: &str, encoding: &str, output: &str, stats: &str) -> Result<String, JsValue> {
    gizza_ai_brotli_decompress_core::run(data, encoding, output, truthy(stats))
        .map_err(|e| JsValue::from_str(&e))
}
