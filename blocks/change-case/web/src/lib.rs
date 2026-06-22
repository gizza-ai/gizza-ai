//! Browser-facing wasm-bindgen wrapper for /tools/change-case/.
//! Compiled with wasm-pack for the standalone /tools/change-case/ page.
use wasm_bindgen::prelude::*;

/// Convert `text` to the chosen `mode` (e.g. `upper`, `snake`, `camel`). The
/// page passes the select value as a string; blank → `upper`. Throws a JS error
/// string on an unknown mode.
#[wasm_bindgen]
pub fn run(text: &str, mode: &str) -> Result<String, JsValue> {
    gizza_ai_change_case_core::convert(text, mode).map_err(|e| JsValue::from_str(&e))
}
