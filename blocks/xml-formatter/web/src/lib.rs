//! Browser-facing wasm-bindgen wrapper for /tools/xml-formatter/.
//! Field order MUST match meta.toml: xml, mode, indent.
use gizza_ai_xml_formatter_core::{format, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(xml: &str, mode: &str, indent: &str) -> Result<String, JsValue> {
    let mode = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let indent: usize = indent.trim().parse().unwrap_or(2);
    format(xml, mode, indent).map_err(|e| JsValue::from_str(&e))
}
