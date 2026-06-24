//! Browser-facing wasm-bindgen wrapper for /tools/html-form-generator/.
use wasm_bindgen::prelude::*;

/// Generate HTML form markup. The standalone page passes every field value as a
/// string, so `styled` arrives as a string and is parsed here (truthy → on).
#[wasm_bindgen]
pub fn run(
    fields: &str,
    method: &str,
    action: &str,
    submit_label: &str,
    styled: &str,
) -> Result<String, JsValue> {
    let styled = matches!(
        styled.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_html_form_generator_core::generate(fields, method, action, submit_label, styled)
        .map_err(|e| JsValue::from_str(&e))
}
