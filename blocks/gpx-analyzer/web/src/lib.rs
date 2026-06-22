//! Browser-facing wasm-bindgen wrapper for /tools/gpx-analyzer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(gpx: &str) -> Result<String, JsValue> {
    gizza_ai_gpx_analyzer_core::render(gpx).map_err(|e| JsValue::from_str(&e))
}
