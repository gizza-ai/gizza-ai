//! Browser-facing wasm-bindgen wrapper for /tools/mongodb-query-to-sql/.
use wasm_bindgen::prelude::*;

/// Checkbox controls arrive as strings; anything that is not an affirmative is false.
fn parse_bool_field(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    query: &str,
    output: &str,
    dialect: &str,
    table: &str,
    nested: &str,
    quote_identifiers: &str,
    rename_id: &str,
) -> Result<String, JsValue> {
    gizza_ai_mongodb_query_to_sql_core::run(
        query,
        output,
        dialect,
        table,
        nested,
        parse_bool_field(quote_identifiers),
        parse_bool_field(rename_id),
    )
    .map_err(|e| JsValue::from_str(&e))
}
