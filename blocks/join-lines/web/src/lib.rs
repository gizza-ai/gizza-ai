//! Browser-facing wasm-bindgen wrapper for /tools/join-lines/.
//! Field order MUST match meta.toml: text, separator, trim, remove_blank,
//! prefix, suffix. Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_join_lines_core::render;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    separator: &str,
    trim: &str,
    remove_blank: &str,
    prefix: &str,
    suffix: &str,
) -> Result<String, JsValue> {
    render(
        text,
        separator,
        truthy(trim),
        truthy(remove_blank),
        prefix,
        suffix,
    )
    .map_err(|e| JsValue::from_str(&e))
}
