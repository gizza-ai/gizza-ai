//! Browser-facing wasm-bindgen wrapper for /tools/json-escape/.
//! Field order MUST match meta.toml: text, mode, quotes. Fields are strings.
use gizza_ai_json_escape_core::{process, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, mode: &str, quotes: &str) -> Result<String, JsValue> {
    let m = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let q = matches!(quotes.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    process(text, m, q).map_err(|e| JsValue::from_str(&e))
}
