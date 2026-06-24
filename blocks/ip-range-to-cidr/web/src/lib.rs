//! Browser-facing wasm-bindgen wrapper for /tools/ip-range-to-cidr/.
//! Compiled with wasm-pack for the standalone /tools/ip-range-to-cidr/ page.
use wasm_bindgen::prelude::*;

/// Convert a start-end IP range into the minimal CIDR cover (or a count).
///
/// The standalone tool page passes every field value as a string. Throws a JS
/// error string on invalid input.
#[wasm_bindgen]
pub fn run(input: &str, output: &str) -> Result<String, JsValue> {
    gizza_ai_ip_range_to_cidr_core::run(input, output).map_err(|e| JsValue::from_str(&e))
}
