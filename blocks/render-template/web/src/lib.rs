//! Browser-facing wasm-bindgen wrapper for /tools/render-template/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(template: &str, data: &str, engine: &str, strict: &str) -> Result<String, JsValue> {
    let strict = matches!(strict, "true" | "1" | "on" | "yes");
    gizza_ai_render_template_core::render(template, data, engine, strict)
        .map_err(|e| JsValue::from_str(&e))
}
