//! Browser-facing wasm-bindgen wrapper for /tools/sql-linter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    sql: &str,
    dialect: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_sql_linter_core::lint(sql, dialect, min_severity, ignore, format)
        .map_err(|e| JsValue::from_str(&e))
}
