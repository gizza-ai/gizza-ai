//! Browser-facing wasm-bindgen wrapper for /tools/js-css-minifier/.
//! Field order MUST match meta.toml: code, language, remove_comments, report.
//! Fields arrive as strings.
use gizza_ai_js_css_minifier_core::{minify, Language};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    code: &str,
    language: &str,
    remove_comments: &str,
    report: &str,
) -> Result<String, JsValue> {
    minify(
        code,
        Language::parse(language),
        truthy(remove_comments),
        truthy(report),
    )
    .map_err(|e| JsValue::from_str(&e))
}
