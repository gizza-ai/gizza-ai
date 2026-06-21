//! Browser-facing wasm-bindgen wrapper for /tools/text-statistics/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str) -> Result<String, JsValue> {
    Ok(gizza_ai_text_statistics_core::summary(text))
}
