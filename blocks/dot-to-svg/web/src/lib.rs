//! Browser-facing wasm-bindgen wrapper for /tools/dot-to-svg/.
use gizza_ai_dot_to_svg_core::{render, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(dot: &str, dark_mode: &str) -> Result<String, JsValue> {
    let dark = matches!(dark_mode, "true" | "1" | "on" | "yes");
    render(dot, &Options { dark_mode: dark }).map_err(|e| JsValue::from_str(&e))
}
