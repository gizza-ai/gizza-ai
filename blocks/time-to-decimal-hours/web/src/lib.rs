//! Browser-facing wasm-bindgen wrapper for /tools/time-to-decimal-hours/.
//! Compiled with wasm-pack for the standalone /tools/time-to-decimal-hours/ page.
//!
//! Field order MUST match meta.toml: value, mode.
use wasm_bindgen::prelude::*;

/// Convert `value` to/from decimal hours and return a pretty-printed JSON
/// object. On a parse error it throws the error string.
#[wasm_bindgen]
pub fn run(value: &str, mode: &str) -> Result<String, JsValue> {
    let mode = if mode.trim().is_empty() { "auto" } else { mode };
    gizza_ai_time_to_decimal_hours_core::convert_json(value, mode)
        .map_err(|e| JsValue::from_str(&e))
}
