//! Browser-facing wasm-bindgen wrapper for /tools/flatten-json/.
//! The argument order mirrors `page/meta.toml`'s `[[input]]` order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    json: &str,
    direction: &str,
    separator: &str,
    array_notation: &str,
    max_depth: u32,
    flatten_arrays: bool,
    preserve_empty: bool,
    key_case: &str,
    output: &str,
    pretty: bool,
    indent: u32,
) -> Result<String, JsValue> {
    gizza_ai_flatten_json_core::convert(
        json,
        direction,
        separator,
        array_notation,
        max_depth as usize,
        flatten_arrays,
        preserve_empty,
        key_case,
        output,
        pretty,
        indent as usize,
    )
    .map_err(|e| JsValue::from_str(&e))
}
