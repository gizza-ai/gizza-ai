//! Browser-facing wasm-bindgen wrapper for /tools/outline-to-mindmap/.
//! tool.js passes every field's value as a raw string, so each param is parsed
//! here and funneled through the shared core validation.
use gizza_ai_outline_to_mindmap_core::{render, Direction, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    outline: &str,
    direction: &str,
    colorful: &str,
    dark_mode: &str,
    title: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        direction: Direction::parse(direction),
        colorful: matches!(colorful, "true" | "1" | "on" | "yes"),
        dark_mode: matches!(dark_mode, "true" | "1" | "on" | "yes"),
        title: if title.trim().is_empty() { "Mind Map".into() } else { title.to_string() },
    };
    render(outline, &opts).map_err(|e| JsValue::from_str(&e))
}
