//! Browser-facing wasm-bindgen wrapper for /tools/sql-schema-extractor/.
//! Compiled with wasm-pack for the standalone page.
//!
//! The standalone tool page passes every field value as a string, so the
//! boolean params arrive as strings and are parsed here (positive-truthy →
//! true); the core owns all validation.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Parse a SQL dump's DDL into a table/column/constraint model.
///
/// - `sql`: the SQL / DDL text (CREATE TABLE / ALTER TABLE / CREATE INDEX).
/// - `output`: `json` | `markdown`.
/// - `dialect`: `auto` | `mysql` | `postgres` | `sqlite` | `mssql` | `generic`.
/// - `apply_alter` / `include_indexes`:
///   `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
///
/// Throws a JS error string on invalid input or an invalid option.
#[wasm_bindgen]
pub fn run(
    sql: &str,
    output: &str,
    dialect: &str,
    apply_alter: &str,
    include_indexes: &str,
) -> Result<String, JsValue> {
    gizza_ai_sql_schema_extractor_core::extract(
        sql,
        output,
        dialect,
        truthy(apply_alter),
        truthy(include_indexes),
    )
    .map_err(|e| JsValue::from_str(&e))
}
