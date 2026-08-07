//! Browser-facing wasm-bindgen wrapper for /tools/typescript-to-json-schema/.
//! Field order MUST match meta.toml: typescript, root_type, draft, required,
//! additional_properties, jsdoc.
use gizza_ai_typescript_to_json_schema_core::{convert, Draft, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    typescript: &str,
    root_type: &str,
    draft: &str,
    required: &str,
    additional_properties: &str,
    jsdoc: &str,
) -> Result<String, JsValue> {
    let draft = match draft.trim() {
        "draft-07" | "draft7" | "07" | "7" => Draft::Draft07,
        _ => Draft::Draft2020,
    };
    let opts = Options {
        draft,
        root_type: root_type.to_string(),
        required: truthy(required),
        additional_properties: truthy(additional_properties),
        jsdoc: truthy(jsdoc),
    };
    convert(typescript, &opts).map_err(|e| JsValue::from_str(&e))
}
