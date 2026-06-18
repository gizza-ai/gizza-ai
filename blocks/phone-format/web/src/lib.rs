//! Browser-facing wasm-bindgen wrapper for /tools/phone-format/.
//! Compiled with wasm-pack for the standalone /tools/phone-format/ page.
//!
//! The page driver (`tool.js`) calls this with the field values in declared
//! order — so the parameter order MUST match `page/meta.toml`'s inputs:
//! `number` then `region`.
use wasm_bindgen::prelude::*;

/// Parse, validate, and format `number` (interpreting it with the optional
/// ISO-3166 `region` hint). Throws a JS error string on failure.
#[wasm_bindgen]
pub fn run(number: &str, region: &str) -> Result<String, JsValue> {
    gizza_ai_phone_format_core::format_number(number, region).map_err(|e| JsValue::from_str(&e))
}
