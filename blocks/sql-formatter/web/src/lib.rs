//! Browser-facing wasm-bindgen wrapper for /tools/sql-formatter/.
//! Field order MUST match meta.toml: sql, indent, keyword_case. Fields arrive as strings.
use gizza_ai_sql_formatter_core::{format, KeywordCase};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(sql: &str, indent: &str, keyword_case: &str) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    let case = KeywordCase::parse(keyword_case).map_err(|e| JsValue::from_str(&e))?;
    format(sql, n, case).map_err(|e| JsValue::from_str(&e))
}
