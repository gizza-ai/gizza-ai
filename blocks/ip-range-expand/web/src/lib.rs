//! Browser-facing wasm-bindgen wrapper for /tools/ip-range-expand/.
//! Compiled with wasm-pack for the standalone /tools/ip-range-expand/ page.
use wasm_bindgen::prelude::*;

/// Expand a CIDR or IP range into the address list (or count).
///
/// The standalone tool page passes every field value as a string, so `limit`
/// arrives as a string and is parsed here (blank/unparseable → the default).
/// Throws a JS error string on invalid input.
#[wasm_bindgen]
pub fn run(input: &str, output: &str, limit: &str) -> Result<String, JsValue> {
    let limit = limit
        .trim()
        .parse::<u64>()
        .unwrap_or(gizza_ai_ip_range_expand_core::DEFAULT_LIMIT);
    gizza_ai_ip_range_expand_core::expand(input, output, limit).map_err(|e| JsValue::from_str(&e))
}
