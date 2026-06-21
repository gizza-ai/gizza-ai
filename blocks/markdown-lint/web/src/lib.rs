//! Browser-facing wasm-bindgen wrapper for /tools/markdown-lint/.
//! Field order MUST match meta.toml: markdown, mode. Fields arrive as strings.
use gizza_ai_markdown_lint_core::run as lint_run;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(markdown: &str, mode: &str) -> Result<String, JsValue> {
    let m = if mode.trim().is_empty() { "check" } else { mode.trim() };
    lint_run(markdown, m).map_err(|e| JsValue::from_str(&e))
}
