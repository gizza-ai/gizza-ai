//! Browser-facing wasm-bindgen wrapper for /tools/svg-recolor/.
//! Field order MUST match meta.toml: svg, mapping, to. All fields are strings.
use gizza_ai_svg_recolor_core::run as recolor_run;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(svg: &str, mapping: &str, to: &str) -> Result<String, JsValue> {
    recolor_run(svg, mapping, to).map_err(|e| JsValue::from_str(&e))
}
