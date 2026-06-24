//! Browser-facing wasm-bindgen wrapper for /tools/php-serialize/.
//! Compiled with wasm-pack for the standalone /tools/php-serialize/ page.
use wasm_bindgen::prelude::*;

/// Convert a JSON string into a PHP `serialize()` string.
///
/// Throws a JS error string when `json` is not valid JSON.
#[wasm_bindgen]
pub fn run(json: &str) -> Result<String, JsValue> {
    gizza_ai_php_serialize_core::json_to_php(json).map_err(|e| JsValue::from_str(&e))
}
