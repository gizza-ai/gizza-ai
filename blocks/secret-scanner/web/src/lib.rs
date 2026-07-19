//! Browser-facing wasm-bindgen wrapper for /tools/secret-scanner/.
//! Field order MUST match meta.toml: text, min_severity, redact, format.
//! Fields arrive as strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    min_severity: &str,
    redact: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_secret_scanner_core::run(text, min_severity, truthy(redact), format)
        .map_err(|e| JsValue::from_str(&e))
}
