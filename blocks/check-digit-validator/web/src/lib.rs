//! Browser-facing wasm-bindgen wrapper for /tools/check-digit-validator/.
//! The page marshals every field as a string (checkboxes arrive as "true"/"false"),
//! so booleans are parsed here and the core is handed typed values.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(value: &str, scheme: &str, mode: &str, show_steps: &str) -> Result<String, JsValue> {
    let scheme = if scheme.trim().is_empty() { "auto" } else { scheme };
    let mode = if mode.trim().is_empty() { "validate" } else { mode };
    gizza_ai_check_digit_validator_core::run(value, scheme, mode, truthy(show_steps))
        .map(|r| r.to_text())
        .map_err(|e| JsValue::from_str(&e))
}
