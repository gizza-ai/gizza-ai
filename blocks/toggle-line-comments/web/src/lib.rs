//! Browser-facing wasm-bindgen wrapper for /tools/toggle-line-comments/.
//! Field order MUST match meta.toml: code, language, mode, marker,
//! space_after_marker, align, comment_blank_lines. Fields arrive as strings
//! (checkboxes send "true"/"false").
use gizza_ai_toggle_line_comments_core::{render, Align, Mode};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    code: &str,
    language: &str,
    mode: &str,
    marker: &str,
    space_after_marker: &str,
    align: &str,
    comment_blank_lines: &str,
) -> Result<String, JsValue> {
    // Empty select/checkbox fields fall back to the schema defaults.
    let lang = if language.trim().is_empty() { "auto" } else { language };
    let mode = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let align = Align::parse(align).map_err(|e| JsValue::from_str(&e))?;
    render(
        code,
        lang,
        mode,
        marker,
        truthy(space_after_marker),
        align,
        truthy(comment_blank_lines),
    )
    .map_err(|e| JsValue::from_str(&e))
}
