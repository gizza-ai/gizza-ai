//! Browser-facing wasm-bindgen wrapper for /tools/json-array-pluck-field/.
//! Argument order MUST match the descriptor + meta.toml field order:
//! json, field, root, format, delimiter, quote, missing, complex_values, unique.
//! Fields arrive as strings; checkboxes as "true"/"false".
use gizza_ai_json_array_pluck_field_core::{pluck, Complex, Format, Missing, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    json: &str,
    field: &str,
    root: &str,
    format: &str,
    delimiter: &str,
    quote: &str,
    missing: &str,
    complex_values: &str,
    unique: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        field: field.to_string(),
        root: root.to_string(),
        format: Format::parse(format),
        // Not trimmed: a delimiter of " | " (or a lone space) is meaningful.
        delimiter: delimiter.to_string(),
        quote: truthy(quote),
        missing: Missing::parse(missing),
        complex: Complex::parse(complex_values),
        unique: truthy(unique),
    };
    pluck(json, &opts).map_err(|e| JsValue::from_str(&e))
}
