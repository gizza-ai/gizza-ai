//! Browser-facing wasm-bindgen wrapper for /tools/fit-file-decoder/.
use gizza_ai_fit_file_decoder_core::decode_str;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, format: &str) -> Result<String, JsValue> {
    let fmt = if format.trim().is_empty() {
        "summary"
    } else {
        format
    };
    decode_str(data, fmt).map_err(|e| JsValue::from_str(&e))
}
