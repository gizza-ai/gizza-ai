//! Browser-facing wasm-bindgen wrapper for /tools/json-to-types/.
//! Field order MUST match meta.toml: json, output_language, root_name,
//! optional_strategy, json_annotations, export.
use gizza_ai_json_to_types_core::{generate, Language, OptionalStrategy, Options};
use wasm_bindgen::prelude::*;

/// The page hands every field over as a string, so parse the booleans here.
/// Checkboxes arrive as "true"/"false"; be liberal about the other spellings.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    output_language: &str,
    root_name: &str,
    optional_strategy: &str,
    json_annotations: &str,
    export: &str,
) -> Result<String, JsValue> {
    let err = |e: String| JsValue::from_str(&e);
    let opts = Options {
        language: Language::parse(output_language).map_err(err)?,
        root_name: if root_name.trim().is_empty() { "Root".to_string() } else { root_name.to_string() },
        optional_strategy: OptionalStrategy::parse(optional_strategy).map_err(err)?,
        json_annotations: truthy(json_annotations),
        export: truthy(export),
    };
    generate(json, &opts).map_err(err)
}
