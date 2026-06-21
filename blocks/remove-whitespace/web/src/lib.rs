//! Browser-facing wasm-bindgen wrapper for /tools/remove-whitespace/.
//! Field order MUST match meta.toml: text, mode, collapse_blank_lines.
use gizza_ai_remove_whitespace_core::{clean, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, mode: &str, collapse_blank_lines: &str) -> Result<String, JsValue> {
    // The page passes an empty field when the user hasn't picked a mode; default
    // to "trim" so the tool works out of the box (matches the descriptor default).
    let mode = if mode.trim().is_empty() { "trim" } else { mode };
    let mode = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let collapse = matches!(
        collapse_blank_lines.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    Ok(clean(text, mode, collapse))
}
