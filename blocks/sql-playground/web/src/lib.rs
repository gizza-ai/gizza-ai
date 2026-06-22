//! Browser-facing wasm-bindgen wrapper for /tools/sql-playground/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(sql: &str, format: &str) -> Result<String, JsValue> {
    let fmt = if format.is_empty() { "table" } else { format };
    gizza_ai_sql_playground_core::run(sql, fmt).map_err(|e| JsValue::from_str(&e))
}
