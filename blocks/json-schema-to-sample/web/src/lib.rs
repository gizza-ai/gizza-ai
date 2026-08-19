//! Browser-facing wasm-bindgen wrapper for /tools/json-schema-to-sample/.
use gizza_ai_json_schema_to_sample_core::{sample, Options, MAX_ARRAY_ITEMS};
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false"; treat anything else as on.
fn truthy_default_on(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

fn truthy_default_off(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    schema: &str,
    include_optional: &str,
    array_items: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let array_items = if array_items.trim().is_empty() {
        1
    } else {
        array_items.trim().parse::<u32>().map_err(|_| {
            JsValue::from_str(&format!(
                "array_items must be a whole number between 0 and {MAX_ARRAY_ITEMS}"
            ))
        })?
    };
    sample(
        schema,
        Options {
            include_optional: truthy_default_off(include_optional),
            array_items,
            pretty: truthy_default_on(pretty),
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
