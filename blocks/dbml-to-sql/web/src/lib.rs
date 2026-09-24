//! Browser-facing wasm-bindgen wrapper for /tools/dbml-to-sql/.
//! Field order MUST match meta.toml: dbml, dialect, foreign_keys, indexes,
//! comments, if_not_exists, drop_if_exists, quote_identifiers.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    dbml: &str,
    dialect: &str,
    foreign_keys: &str,
    indexes: &str,
    comments: &str,
    if_not_exists: &str,
    drop_if_exists: &str,
    quote_identifiers: &str,
) -> Result<String, JsValue> {
    gizza_ai_dbml_to_sql_core::convert(
        dbml,
        dialect,
        truthy(foreign_keys),
        truthy(indexes),
        truthy(comments),
        truthy(if_not_exists),
        truthy(drop_if_exists),
        truthy(quote_identifiers),
    )
    .map_err(|e| JsValue::from_str(&e))
}
