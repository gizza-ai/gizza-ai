//! Browser-facing wasm-bindgen wrapper for /tools/remove-control-chars/.
//! Field order MUST match meta.toml: text, keep_tabs, keep_newlines, replacement.
use gizza_ai_remove_control_chars_core::remove_control_chars;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    keep_tabs: &str,
    keep_newlines: &str,
    replacement: &str,
) -> Result<String, JsValue> {
    Ok(remove_control_chars(
        text,
        truthy(keep_tabs),
        truthy(keep_newlines),
        replacement,
    ))
}
