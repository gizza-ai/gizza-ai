//! Browser-facing wasm-bindgen wrapper for /tools/basic-auth-header-generator/.
//! tool.js passes every field as a string; parse the bool here. Param order MUST
//! match page/meta.toml.
use gizza_ai_basic_auth_header_generator_core::build;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(username: &str, password: &str, full_header: &str) -> Result<String, JsValue> {
    let full = matches!(full_header.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    build(username, password, full).map_err(|e| JsValue::from_str(&e))
}
