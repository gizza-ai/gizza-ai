//! Browser-facing wasm-bindgen wrapper for /tools/cookie-string-to-json/.
//! The page passes every field value as a string: `decode` is a checkbox
//! (default checked → "true"), `output` is a `<select>` ("object"/"pairs").
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(cookie: &str, decode: &str, output: &str) -> Result<String, JsValue> {
    let decode = matches!(
        decode.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_cookie_string_to_json_core::run(cookie, decode, output)
        .map_err(|e| JsValue::from_str(&e))
}
