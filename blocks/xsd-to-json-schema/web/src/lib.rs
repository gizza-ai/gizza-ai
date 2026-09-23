//! Browser-facing wasm-bindgen wrapper for /tools/xsd-to-json-schema/.
//! Field order MUST match meta.toml: xsd, root_element, draft, attribute_prefix,
//! text_property, required_from_occurs, additional_properties, annotations.
use gizza_ai_xsd_to_json_schema_core::{convert, draft_from_str, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    xsd: &str,
    root_element: &str,
    draft: &str,
    attribute_prefix: &str,
    text_property: &str,
    required_from_occurs: &str,
    additional_properties: &str,
    annotations: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        draft: draft_from_str(draft),
        root_element: root_element.to_string(),
        attribute_prefix: attribute_prefix.to_string(),
        text_property: text_property.to_string(),
        required_from_occurs: truthy(required_from_occurs),
        additional_properties: truthy(additional_properties),
        annotations: truthy(annotations),
    };
    convert(xsd, &opts).map_err(|e| JsValue::from_str(&e))
}
