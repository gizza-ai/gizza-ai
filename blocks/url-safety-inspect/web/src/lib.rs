//! Browser-facing wasm-bindgen wrapper for /tools/url-safety-inspect/.
//! tool.js passes the page field as a raw string; this export takes `&str` and returns
//! the rendered risk report. Param name/order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(url: &str) -> Result<String, JsValue> {
    gizza_ai_url_safety_inspect_core::run(url).map_err(|e| JsValue::from_str(&e))
}
