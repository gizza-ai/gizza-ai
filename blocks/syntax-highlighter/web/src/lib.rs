//! Browser-facing wasm-bindgen wrapper for /tools/syntax-highlighter/.
//! Field order MUST match meta.toml: code, language, theme, format. Fields
//! arrive as strings. Returns the highlighted HTML (or ANSI) as a string.
use gizza_ai_syntax_highlighter_core::{highlight, parse_format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(code: &str, language: &str, theme: &str, format: &str) -> Result<String, JsValue> {
    let fmt = parse_format(format).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let language = (!language.trim().is_empty()).then_some(language);
    let theme = (!theme.trim().is_empty()).then_some(theme);
    highlight(code, language, theme, fmt).map_err(|e| JsValue::from_str(&e.to_string()))
}
