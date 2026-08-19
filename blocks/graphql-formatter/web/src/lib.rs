//! Browser-facing wasm-bindgen wrapper for /tools/graphql-formatter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    indent: &str,
    mode: &str,
    sort_fields: &str,
    remove_comments: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        )
    };
    gizza_ai_graphql_formatter_core::run(
        input,
        indent,
        mode,
        truthy(sort_fields),
        truthy(remove_comments),
    )
    .map_err(|e| JsValue::from_str(&e))
}
