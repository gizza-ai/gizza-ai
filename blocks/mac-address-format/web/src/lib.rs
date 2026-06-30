//! Browser-facing wasm-bindgen wrapper for /tools/mac-address-format/.
use gizza_ai_mac_address_format_core::render;
use wasm_bindgen::prelude::*;

/// Reformat the MAC address(es) in `input` to `format`
/// (`"colon"` default | `"hyphen"` | `"cisco"` | `"bare"`) and `case`
/// (`"lower"` default | `"upper"`).
#[wasm_bindgen]
pub fn run(input: &str, format: &str, case: &str) -> Result<String, JsValue> {
    render(input, format, case).map_err(|e| JsValue::from_str(&e))
}
