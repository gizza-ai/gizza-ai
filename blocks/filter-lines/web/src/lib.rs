//! Browser-facing wasm-bindgen wrapper for /tools/filter-lines/.
//! Field order MUST match meta.toml: text, pattern, mode, regex, ignore_case.
//! Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_filter_lines_core::{render, Mode};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    pattern: &str,
    mode: &str,
    regex: &str,
    ignore_case: &str,
) -> Result<String, JsValue> {
    let m = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    render(text, pattern, m, truthy(regex), truthy(ignore_case)).map_err(|e| JsValue::from_str(&e))
}
