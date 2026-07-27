//! Browser-facing wasm-bindgen wrapper for /tools/remove-empty-lines/.
//! Field order MUST match meta.toml: text, mode, whitespace_only, trim_lines.
//! Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_remove_empty_lines_core::{render, Mode};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    whitespace_only: &str,
    trim_lines: &str,
) -> Result<String, JsValue> {
    let m = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    Ok(render(text, m, truthy(whitespace_only), truthy(trim_lines)))
}
