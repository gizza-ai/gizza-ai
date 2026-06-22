//! Browser-facing wasm-bindgen wrapper for /tools/timezone-convert/.
//! Compiled with wasm-pack for the standalone /tools/timezone-convert/ page.
use wasm_bindgen::prelude::*;

/// Convert `datetime` (a wall-clock time in `from`) into the `to` timezone.
///
/// The standalone tool page passes every field value as a string. Returns a
/// pretty-printed JSON object (see core::Converted). Throws a JS error string on
/// an invalid datetime, an unknown zone, or a non-existent DST-gap time.
#[wasm_bindgen]
pub fn run(datetime: &str, from: &str, to: &str) -> Result<String, JsValue> {
    gizza_ai_timezone_convert_core::render(datetime, from, to)
        .map_err(|e| JsValue::from_str(&e))
}
