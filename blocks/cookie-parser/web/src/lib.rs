//! Browser-facing wasm-bindgen wrapper for /tools/cookie-parser/.
//! The page passes every field value as a string: `mode`/`format` are `<select>`
//! values, `decode`/`warnings` are checkboxes (default checked → "true") and
//! `raw_attributes` is a checkbox that defaults to unchecked ("false").
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[wasm_bindgen]
pub fn run(
    cookie: &str,
    mode: &str,
    format: &str,
    decode: &str,
    raw_attributes: &str,
    warnings: &str,
) -> Result<String, JsValue> {
    gizza_ai_cookie_parser_core::run(
        cookie,
        mode,
        format,
        truthy(decode),
        truthy(raw_attributes),
        truthy(warnings),
    )
    .map_err(|e| JsValue::from_str(&e))
}
