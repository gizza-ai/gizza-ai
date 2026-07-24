//! Browser-facing wasm-bindgen wrapper for /tools/json-to-sql-insert/.
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

/// Generate SQL INSERT statements from a JSON object or array of row objects.
///
/// - `json`: the input data (object → one row; array → one row per object).
/// - `table`: target table name (blank → `my_table`).
/// - `dialect`: `mysql` | `postgres` | `sqlite` | `mssql` | `ansi`.
/// - `values`: `literal` | `placeholder`.
/// - `multi_row` / `create_table` / `drop_table` / `quote_identifiers` /
///   `sort_columns`: `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
/// - `primary_key`: column to mark PRIMARY KEY when `create_table` is on.
/// - `null_handling`: `null` | `default` | `empty-string`.
///
/// Throws a JS error string on invalid JSON or an invalid option.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    json: &str,
    table: &str,
    dialect: &str,
    values: &str,
    multi_row: &str,
    create_table: &str,
    drop_table: &str,
    primary_key: &str,
    quote_identifiers: &str,
    null_handling: &str,
    sort_columns: &str,
) -> Result<String, JsValue> {
    gizza_ai_json_to_sql_insert_core::generate_from_str(
        json,
        table,
        dialect,
        values,
        truthy(multi_row),
        truthy(create_table),
        truthy(drop_table),
        primary_key,
        truthy(quote_identifiers),
        null_handling,
        truthy(sort_columns),
    )
    .map_err(|e| JsValue::from_str(&e))
}
