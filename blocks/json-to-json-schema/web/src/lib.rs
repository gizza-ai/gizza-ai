//! Browser-facing wasm-bindgen wrapper for /tools/json-to-json-schema/.
//! Field order MUST match meta.toml: json, draft, additional_properties, required,
//! detect_formats, title.
use gizza_ai_json_to_json_schema_core::{infer, Draft, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    draft: &str,
    additional_properties: &str,
    required: &str,
    detect_formats: &str,
    title: &str,
) -> Result<String, JsValue> {
    let draft = match draft.trim() {
        "draft-07" | "draft7" | "07" | "7" => Draft::Draft07,
        _ => Draft::Draft2020,
    };
    let opts = Options {
        draft,
        additional_properties: truthy(additional_properties),
        required: truthy(required),
        detect_formats: truthy(detect_formats),
        title: title.to_string(),
    };
    infer(json, &opts).map_err(|e| JsValue::from_str(&e))
}
