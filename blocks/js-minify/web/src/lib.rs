//! Browser-facing wasm-bindgen wrapper for /tools/js-minify/.
//! Field order MUST match meta.toml: code, remove_comments, keep_license.
//! Fields arrive as strings.
use gizza_ai_js_minify_core::minify_with;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(code: &str, remove_comments: &str, keep_license: &str) -> Result<String, JsValue> {
    minify_with(code, truthy(remove_comments), truthy(keep_license))
        .map_err(|e| JsValue::from_str(&e))
}
