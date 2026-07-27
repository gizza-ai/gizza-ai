//! Browser-facing wasm-bindgen wrapper for /tools/json-to-xml/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    json: &str,
    root_element: &str,
    array_item_element: &str,
    format: &str,
    indent: &str,
    xml_declaration: &str,
    attribute_prefix: &str,
    text_key: &str,
) -> Result<String, JsValue> {
    let indent = indent.trim().parse::<i64>().unwrap_or(2).clamp(0, 8) as usize;
    let opt = gizza_ai_json_to_xml_core::Options {
        root_element: defaulted(root_element, "root"),
        array_item_element: defaulted(array_item_element, "item"),
        pretty: format.trim() != "compact",
        indent,
        xml_declaration: matches!(
            xml_declaration.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        ),
        attribute_prefix: attribute_prefix.to_string(),
        text_key: defaulted(text_key, "#text"),
    };
    gizza_ai_json_to_xml_core::to_xml(json, &opt).map_err(|e| JsValue::from_str(&e))
}

fn defaulted(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}
