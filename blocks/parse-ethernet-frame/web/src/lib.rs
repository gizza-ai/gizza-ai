//! Browser-facing wasm-bindgen wrapper for /tools/parse-ethernet-frame/.
use gizza_ai_parse_ethernet_frame_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(frame: &str) -> Result<String, JsValue> {
    render(frame).map_err(|e| JsValue::from_str(&e))
}
