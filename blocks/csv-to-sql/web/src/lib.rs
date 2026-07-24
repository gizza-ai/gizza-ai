//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-sql/.
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

/// Generate SQL CREATE TABLE + INSERT statements from a CSV or JSON table.
///
/// - `input`: the table data (CSV/delimited text, or JSON object/array).
/// - `format`: `auto` | `csv` | `json`.
/// - `delimiter`: `auto` | `comma` | `tab` | `semicolon` | `pipe` (CSV only).
/// - `has_header` / `multi_row` / `create_table` / `drop_table` /
///   `quote_identifiers` / `infer_types` / `detect_dates`:
///   `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
/// - `table`: target table name (blank → `my_table`).
/// - `dialect`: `mysql` | `postgres` | `sqlite` | `mssql` | `ansi`.
/// - `values`: `literal` | `placeholder`.
/// - `primary_key`: column to mark PRIMARY KEY when `create_table` is on.
/// - `null_handling`: `null` | `default` | `empty-string`.
///
/// Throws a JS error string on invalid input or an invalid option.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    format: &str,
    delimiter: &str,
    has_header: &str,
    table: &str,
    dialect: &str,
    values: &str,
    multi_row: &str,
    create_table: &str,
    drop_table: &str,
    primary_key: &str,
    quote_identifiers: &str,
    null_handling: &str,
    infer_types: &str,
    detect_dates: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_to_sql_core::generate_from_str(
        input,
        format,
        delimiter,
        truthy(has_header),
        table,
        dialect,
        values,
        truthy(multi_row),
        truthy(create_table),
        truthy(drop_table),
        primary_key,
        truthy(quote_identifiers),
        null_handling,
        truthy(infer_types),
        truthy(detect_dates),
    )
    .map_err(|e| JsValue::from_str(&e))
}
