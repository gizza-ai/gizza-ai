//! Browser-facing wasm-bindgen wrapper for /tools/data-reshape/.
//! Field order MUST match meta.toml: data, query, input_format, output_format, pretty.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    query: &str,
    input_format: &str,
    output_format: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    gizza_ai_data_reshape_core::run(data, query, input_format, output_format, truthy(pretty))
        .map_err(|e| JsValue::from_str(&e))
}
