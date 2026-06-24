//! Browser-facing wasm-bindgen wrapper for /tools/extract-mac-addresses/.
use gizza_ai_extract_mac_addresses_core::render;
use wasm_bindgen::prelude::*;

/// Find every MAC address in `text` and normalize to `format`
/// (`"colon"` default | `"hyphen"` | `"cisco"` | `"bare"`).
#[wasm_bindgen]
pub fn run(text: &str, format: &str) -> Result<String, JsValue> {
    render(text, format).map_err(|e| JsValue::from_str(&e))
}
