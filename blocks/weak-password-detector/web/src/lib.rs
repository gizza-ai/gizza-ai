//! Browser-facing wasm-bindgen wrapper for /tools/weak-password-detector/.
use wasm_bindgen::prelude::*;

/// Parse a checkbox/query-param value as a positive-truthy boolean, falling back
/// to `default` when the value is absent/empty (deep-links may omit it).
fn truthy(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(input: &str, case_sensitive: &str, normalize_leet: &str) -> Result<String, JsValue> {
    gizza_ai_weak_password_detector_core::render(
        input,
        truthy(case_sensitive, false),
        truthy(normalize_leet, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
