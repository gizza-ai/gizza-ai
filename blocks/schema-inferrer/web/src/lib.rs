//! Browser-facing wasm-bindgen wrapper for /tools/schema-inferrer/.
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

/// Infer a JSON Schema and/or a SQL CREATE TABLE from a sample dataset.
///
/// - `data`: the sample dataset (CSV/delimited text, or JSON object/array).
/// - `format`: `auto` | `csv` | `json`.
/// - `delimiter`: `auto` | `comma` | `tab` | `semicolon` | `pipe` (CSV only).
/// - `has_header` / `not_null` / `detect_formats`:
///   `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
/// - `output`: `both` | `json-schema` | `sql`.
/// - `table`: table name / schema title (blank → `my_table`).
/// - `dialect`: `mysql` | `postgres` | `sqlite` | `mssql` | `ansi`.
/// - `draft`: `2020-12` | `draft-07`.
/// - `primary_key`: column to mark PRIMARY KEY / required.
///
/// Throws a JS error string on invalid input or an invalid option.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    format: &str,
    delimiter: &str,
    has_header: &str,
    output: &str,
    table: &str,
    dialect: &str,
    draft: &str,
    primary_key: &str,
    not_null: &str,
    detect_formats: &str,
) -> Result<String, JsValue> {
    gizza_ai_schema_inferrer_core::infer_from_str(
        data,
        format,
        delimiter,
        truthy(has_header),
        output,
        table,
        dialect,
        draft,
        primary_key,
        truthy(not_null),
        truthy(detect_formats),
    )
    .map_err(|e| JsValue::from_str(&e))
}
