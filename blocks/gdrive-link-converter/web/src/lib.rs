//! Browser-facing wasm-bindgen wrapper for /tools/gdrive-link-converter/.
//! tool.js passes every page field as a raw string; this export takes `&str`
//! for each and parses the bool here. Param order MUST match page/meta.toml.
use gizza_ai_gdrive_link_converter_core::convert;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, output: &str, size: &str, per_line: &str) -> Result<String, JsValue> {
    let per_line = matches!(per_line.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    convert(input, output, size, per_line).map_err(|e| JsValue::from_str(&e))
}
