//! Browser-facing wasm-bindgen wrapper for /tools/resume-builder/.
use gizza_ai_resume_builder_core::build;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str) -> Result<String, JsValue> {
    build(data).map_err(|e| JsValue::from_str(&e))
}
