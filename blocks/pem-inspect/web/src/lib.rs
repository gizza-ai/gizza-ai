//! Browser-facing wasm-bindgen wrapper for /tools/pem-inspect/.
//! Compiled with wasm-pack for the standalone /tools/pem-inspect/ page. The
//! wasm32-unknown-unknown target has no std clock, so certificate expiry is
//! evaluated against the browser's current time (`Date.now()`). The page passes
//! the pasted PEM text as a single field; output is pretty-printed JSON.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    gizza_ai_pem_inspect_core::run(input, now).map_err(|e| JsValue::from_str(&e))
}
