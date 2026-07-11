//! Browser-facing wasm-bindgen wrapper for /tools/metadata-privacy-linter/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Scan a base64/data-URL image for privacy-sensitive metadata.
#[wasm_bindgen]
pub fn run(
    image_base64: &str,
    min_risk: &str,
    reveal_values: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_metadata_privacy_linter_core::run(
        image_base64,
        min_risk,
        truthy(reveal_values),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
