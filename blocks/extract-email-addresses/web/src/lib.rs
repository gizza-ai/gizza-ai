//! Browser-facing wasm-bindgen wrapper for /tools/extract-email-addresses/.
//! Field order MUST match meta.toml: text, group_by_domain.
//! Page fields arrive as strings (a checkbox sends "true"/"false").
use gizza_ai_extract_email_addresses_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, group_by_domain: &str) -> Result<String, JsValue> {
    let group = matches!(
        group_by_domain.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    render(text, group).map_err(|e| JsValue::from_str(&e))
}
