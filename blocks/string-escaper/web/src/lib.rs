//! Browser-facing wasm-bindgen wrapper for /tools/string-escaper/.
//! The standalone tool page passes every field value as a string; `quotes`
//! arrives as "true"/"false" (checkbox) and is parsed to a bool here.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, target: &str, mode: &str, quotes: &str) -> Result<String, JsValue> {
    let quotes = matches!(
        quotes.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_string_escaper_core::run(text, target, mode, quotes)
        .map_err(|e| JsValue::from_str(&e))
}
