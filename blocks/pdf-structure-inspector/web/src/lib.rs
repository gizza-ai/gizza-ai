//! Browser-facing wasm-bindgen wrapper for /tools/pdf-structure-inspector/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    section: &str,
    object_id: &str,
    filter_key: &str,
    max_objects: &str,
    format: &str,
) -> Result<String, JsValue> {
    let max_objects = if max_objects.trim().is_empty() {
        gizza_ai_pdf_structure_inspector_core::DEFAULT_MAX_OBJECTS
    } else {
        max_objects
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("max_objects must be an integer"))?
    };
    let opts = gizza_ai_pdf_structure_inspector_core::Options {
        section: section.to_string(),
        object_id: object_id.to_string(),
        filter_key: filter_key.to_string(),
        max_objects,
        format: format.to_string(),
    };
    gizza_ai_pdf_structure_inspector_core::run(input, &opts).map_err(|e| JsValue::from_str(&e))
}
