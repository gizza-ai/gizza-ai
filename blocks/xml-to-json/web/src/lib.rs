//! Browser-facing wasm-bindgen wrapper for /tools/xml-to-json/.
//! Field order MUST match meta.toml: xml, attribute_prefix, text_key,
//! attributes, coerce_types.
use gizza_ai_xml_to_json_core::{to_json, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    xml: &str,
    attribute_prefix: &str,
    text_key: &str,
    attributes: &str,
    coerce_types: &str,
) -> Result<String, JsValue> {
    let opt = Options {
        attr_prefix: if attribute_prefix.is_empty() { "@".to_string() } else { attribute_prefix.to_string() },
        text_key: if text_key.is_empty() { "#text".to_string() } else { text_key.to_string() },
        include_attributes: truthy(attributes),
        coerce: truthy(coerce_types),
    };
    to_json(xml, &opt).map_err(|e| JsValue::from_str(&e))
}
