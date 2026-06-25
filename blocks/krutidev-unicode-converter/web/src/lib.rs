//! Browser-facing wasm-bindgen wrapper for /tools/krutidev-unicode-converter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    gizza_ai_krutidev_unicode_converter_core::run(input).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn krutidev_to_unicode(input: &str) -> Result<String, JsValue> {
    gizza_ai_krutidev_unicode_converter_core::krutidev_to_unicode(input).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn unicode_to_krutidev(input: &str) -> Result<String, JsValue> {
    gizza_ai_krutidev_unicode_converter_core::unicode_to_krutidev(input).map_err(|e| JsValue::from_str(&e))
}
