//! Browser-facing wasm-bindgen wrapper for /tools/json-normalize/.
//! The argument order mirrors `page/meta.toml`'s `[[input]]` order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    json: &str,
    schema: &str,
    root: &str,
    path: &str,
    id_field: &str,
    on_missing_id: &str,
    on_conflict: &str,
    output: &str,
    pretty: bool,
    indent: usize,
) -> Result<String, JsValue> {
    gizza_ai_json_normalize_core::normalize(
        json,
        schema,
        root,
        path,
        id_field,
        on_missing_id,
        on_conflict,
        output,
        pretty,
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}
