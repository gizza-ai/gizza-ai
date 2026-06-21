//! Browser-facing wasm-bindgen wrapper for /tools/date-diff/.
//! Compiled with wasm-pack for the standalone /tools/date-diff/ page.
use wasm_bindgen::prelude::*;

/// Compute the duration between `start` and `end`, returning a pretty-printed
/// JSON object (calendar breakdown + flat totals + summary). The page passes the
/// two text-field values; on a parse error it throws the error string.
#[wasm_bindgen]
pub fn run(start: &str, end: &str) -> Result<String, JsValue> {
    gizza_ai_date_diff_core::diff_json(start, end).map_err(|e| JsValue::from_str(&e))
}
