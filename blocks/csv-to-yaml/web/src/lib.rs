//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-yaml/.
//! Field order MUST match meta.toml: data, header, infer_types, delimiter.
use gizza_ai_csv_to_yaml_core::to_yaml;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
pub fn run(data: &str, header: &str, infer_types: &str, delimiter: &str) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    to_yaml(data, truthy(header), truthy(infer_types), delim).map_err(|e| JsValue::from_str(&e))
}
