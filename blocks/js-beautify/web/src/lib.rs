//! Browser-facing wasm-bindgen wrapper for /tools/js-beautify/.
//! Field order MUST match meta.toml: code, indent, indent_char. Fields arrive as strings.
use gizza_ai_js_beautify_core::{beautify_with, IndentChar};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(code: &str, indent: &str, indent_char: &str) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    let ch = if indent_char.trim().eq_ignore_ascii_case("tab") {
        IndentChar::Tab
    } else {
        IndentChar::Space
    };
    beautify_with(code, n, ch).map_err(|e| JsValue::from_str(&e))
}
