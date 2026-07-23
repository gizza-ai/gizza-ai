//! Browser-facing wasm-bindgen wrapper for /tools/http-header-parser/.
//! Field order MUST match meta.toml: headers, case, duplicates. Empty option
//! fields fall back to the schema defaults.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(headers: &str, case: &str, duplicates: &str) -> Result<String, JsValue> {
    let case = if case.is_empty() { "canonical" } else { case };
    let duplicates = if duplicates.is_empty() { "combine" } else { duplicates };
    gizza_ai_http_header_parser_core::run(headers, case, duplicates)
        .map_err(|e| JsValue::from_str(&e))
}
