//! Browser-facing wasm-bindgen wrapper for /tools/prisma-schema-from-sql/.
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

/// Convert SQL DDL into a Prisma schema string.
///
/// - `input`: the SQL / DDL text (CREATE TABLE / ALTER TABLE / CREATE INDEX).
/// - `provider`: `postgresql` | `mysql` | `sqlite` | `sqlserver`.
/// - `header` / `relations` / `native_types` / `map_names`:
///   `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
///
/// Throws a JS error string on invalid input or an invalid option.
#[wasm_bindgen]
pub fn run(
    input: &str,
    provider: &str,
    header: &str,
    relations: &str,
    native_types: &str,
    map_names: &str,
) -> Result<String, JsValue> {
    gizza_ai_prisma_schema_from_sql_core::convert(
        input,
        provider,
        truthy(header),
        truthy(relations),
        truthy(native_types),
        truthy(map_names),
    )
    .map_err(|e| JsValue::from_str(&e))
}
