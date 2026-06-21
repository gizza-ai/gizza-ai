//! Browser-facing wasm-bindgen wrapper for /tools/http-request-builder/.
//! Field order MUST match meta.toml: url, method, headers, body.
use gizza_ai_http_request_builder_core::build;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(url: &str, method: &str, headers: &str, body: &str) -> Result<String, JsValue> {
    build(method, url, headers, body).map_err(|e| JsValue::from_str(&e))
}
