//! Browser-facing wasm-bindgen wrapper for /tools/lznt1-decompress/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, input_encoding: &str, output_encoding: &str) -> Result<String, JsValue> {
    gizza_ai_lznt1_decompress_core::run(data, input_encoding, output_encoding)
        .map_err(|e| JsValue::from_str(&e))
}
