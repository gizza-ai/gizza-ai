//! Browser-facing wasm-bindgen wrapper for /tools/data-uri-encode/.
//! Compiled with wasm-pack for the standalone page. Every field value arrives
//! as a string; `mime` blank → text/plain, `encoding` blank → base64.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, mime: &str, encoding: &str) -> Result<String, JsValue> {
    gizza_ai_data_uri_encode_core::encode(text, mime, encoding)
        .map(|r| r.data_uri)
        .map_err(|e| JsValue::from_str(&e))
}
