//! Browser-facing wasm-bindgen wrapper for /tools/url-cleaner/.
//! tool.js passes every page field as a raw string; this export takes `&str`
//! for each and parses the bool here. Param order MUST match page/meta.toml.
use gizza_ai_url_cleaner_core::clean;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(url: &str, per_line: &str, extra: &str) -> Result<String, JsValue> {
    let per_line = matches!(per_line.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    clean(url, per_line, extra).map_err(|e| JsValue::from_str(&e))
}
