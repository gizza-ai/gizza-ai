//! Browser-facing wasm-bindgen wrapper for /tools/amazon-order-analyzer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(csv: &str, output: &str, top: u32) -> Result<String, JsValue> {
    gizza_ai_amazon_order_analyzer_core::analyze(csv, output, top).map_err(|e| JsValue::from_str(&e))
}
