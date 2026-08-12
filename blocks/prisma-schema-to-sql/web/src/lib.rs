//! Browser-facing wasm-bindgen wrapper for /tools/prisma-schema-to-sql/.
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

/// Convert a Prisma schema into CREATE TABLE DDL.
///
/// - `input`: the `schema.prisma` text.
/// - `dialect`: `auto` | `postgresql` | `mysql` | `sqlite` | `sqlserver`.
/// - `foreign_keys` / `indexes` / `if_not_exists` / `drop_if_exists` /
///   `quote_identifiers`: `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
///
/// Throws a JS error string on invalid input or an invalid option.
#[wasm_bindgen]
pub fn run(
    input: &str,
    dialect: &str,
    foreign_keys: &str,
    indexes: &str,
    if_not_exists: &str,
    drop_if_exists: &str,
    quote_identifiers: &str,
) -> Result<String, JsValue> {
    gizza_ai_prisma_schema_to_sql_core::convert(
        input,
        dialect,
        truthy(foreign_keys),
        truthy(indexes),
        truthy(if_not_exists),
        truthy(drop_if_exists),
        truthy(quote_identifiers),
    )
    .map_err(|e| JsValue::from_str(&e))
}
