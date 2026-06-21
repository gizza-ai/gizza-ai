//! Browser-facing wasm-bindgen wrapper for /tools/xml-to-csv/.
//! Field order MUST match meta.toml: xml, record, attributes, delimiter.
use gizza_ai_xml_to_csv_core::to_csv;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(xml: &str, record: &str, attributes: &str, delimiter: &str) -> Result<String, JsValue> {
    let attrs = matches!(attributes.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    to_csv(xml, record, attrs, delim).map_err(|e| JsValue::from_str(&e))
}
