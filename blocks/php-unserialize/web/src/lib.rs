//! Browser-facing wasm-bindgen wrapper for /tools/php-unserialize/.
//! Compiled with wasm-pack for the standalone /tools/php-unserialize/ page.
use wasm_bindgen::prelude::*;

/// Decode a PHP `serialize()` string into pretty-printed JSON.
///
/// Throws a JS error string when the input is not a valid serialized value.
#[wasm_bindgen]
pub fn run(serialized: &str) -> Result<String, JsValue> {
    gizza_ai_php_unserialize_core::php_to_json(serialized).map_err(|e| JsValue::from_str(&e))
}
