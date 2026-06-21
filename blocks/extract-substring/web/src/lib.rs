//! Browser-facing wasm-bindgen wrapper for /tools/extract-substring/.
//! Field order MUST match meta.toml: text, mode, start, end, start_delim, end_delim.
//! Page fields arrive as strings.
use gizza_ai_extract_substring_core::{render, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    start: &str,
    end: &str,
    start_delim: &str,
    end_delim: &str,
) -> Result<String, JsValue> {
    let m = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let start_i: i64 = start.trim().parse().unwrap_or(0);
    let end_i: Option<i64> = end.trim().parse().ok();
    render(text, m, start_i, end_i, start_delim, end_delim).map_err(|e| JsValue::from_str(&e))
}
