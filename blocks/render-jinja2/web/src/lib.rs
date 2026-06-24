//! Browser-facing wasm-bindgen wrapper for /tools/render-jinja2/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(template: &str, data: &str, data_format: &str, strict: &str) -> Result<String, JsValue> {
    let strict = matches!(strict, "true" | "1" | "on" | "yes");
    gizza_ai_render_jinja2_core::render(template, data, data_format, strict)
        .map_err(|e| JsValue::from_str(&e))
}
