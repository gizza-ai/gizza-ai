//! Browser-facing wasm-bindgen wrapper for /tools/safelink-decoder/.
use gizza_ai_safelink_decoder_core::decode;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(url: &str, per_line: &str) -> Result<String, JsValue> {
    let per_line = matches!(per_line.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    decode(url, per_line).map_err(|e| JsValue::from_str(&e))
}
