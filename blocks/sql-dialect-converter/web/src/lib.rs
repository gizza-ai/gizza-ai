//! Browser-facing wasm-bindgen wrapper for /tools/sql-dialect-converter/.
//! Field order MUST match meta.toml: sql, from, to. Fields are strings.
use gizza_ai_sql_dialect_converter_core::{convert, Dialect};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(sql: &str, from: &str, to: &str) -> Result<String, JsValue> {
    let f = Dialect::parse(from).map_err(|e| JsValue::from_str(&e))?;
    let t = Dialect::parse(to).map_err(|e| JsValue::from_str(&e))?;
    convert(sql, f, t).map_err(|e| JsValue::from_str(&e))
}
