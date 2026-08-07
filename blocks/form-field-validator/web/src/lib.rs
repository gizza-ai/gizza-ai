//! Browser-facing wasm-bindgen wrapper for /tools/form-field-validator/.
//! Field order MUST match meta.toml: fields, country, required, rules,
//! normalize, mask_sensitive, output. Checkboxes arrive as strings.
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    matches!(t.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    fields: &str,
    country: &str,
    required: &str,
    rules: &str,
    normalize: &str,
    mask_sensitive: &str,
    output: &str,
) -> Result<String, JsValue> {
    let country = if country.trim().is_empty() {
        "any"
    } else {
        country
    };
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output
    };
    gizza_ai_form_field_validator_core::run(
        fields,
        country,
        required,
        rules,
        truthy(normalize, true),
        truthy(mask_sensitive, true),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
