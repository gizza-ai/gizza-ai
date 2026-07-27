//! Browser-facing wasm-bindgen wrapper for /tools/code-chunker/.
//! Compiled with wasm-pack for the standalone page. The page passes every field
//! value as a string; a blank max_lines falls back to the descriptor default and
//! the core validates ranges. Arg order matches page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(code: &str, language: &str, max_lines: &str, format: &str) -> Result<String, JsValue> {
    let max_lines = max_lines
        .trim()
        .parse::<u32>()
        .unwrap_or(gizza_ai_code_chunker_core::DEFAULT_MAX_LINES);
    gizza_ai_code_chunker_core::chunk(code, language, max_lines, format)
        .map_err(|e| JsValue::from_str(&e))
}
