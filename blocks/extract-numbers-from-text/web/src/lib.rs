//! Browser-facing wasm-bindgen wrapper for /tools/extract-numbers-from-text/.
//! Field order MUST match meta.toml: text, mode, unique, sort, delimiter, stats.
//! Fields arrive as strings (checkboxes send "true"/"false").
use gizza_ai_extract_numbers_from_text_core::{render, Delimiter, Mode, Sort};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    unique: &str,
    sort: &str,
    delimiter: &str,
    stats: &str,
) -> Result<String, JsValue> {
    // Empty select fields fall back to the schema defaults.
    let mode = Mode::parse(if mode.trim().is_empty() { "all" } else { mode })
        .map_err(|e| JsValue::from_str(&e))?;
    let sort = Sort::parse(if sort.trim().is_empty() { "original" } else { sort })
        .map_err(|e| JsValue::from_str(&e))?;
    let delimiter =
        Delimiter::parse(if delimiter.trim().is_empty() { "newline" } else { delimiter })
            .map_err(|e| JsValue::from_str(&e))?;
    Ok(render(text, mode, truthy(unique), sort, delimiter, truthy(stats)))
}
