//! Browser-facing wasm-bindgen wrapper for /tools/sql-danger-checker/.
//! Field order MUST match meta.toml: sql, dialect, min_severity, strict, allow,
//! format. Fields arrive as strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    sql: &str,
    dialect: &str,
    min_severity: &str,
    strict: &str,
    allow: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_sql_danger_checker_core::run(sql, dialect, min_severity, strict, allow, format)
        .map_err(|e| JsValue::from_str(&e))
}
