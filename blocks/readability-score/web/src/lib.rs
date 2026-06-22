//! Browser-facing wasm-bindgen wrapper for /tools/readability-score/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str) -> Result<String, JsValue> {
    Ok(gizza_ai_readability_score_core::summary(text))
}
