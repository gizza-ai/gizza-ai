//! Browser-facing wasm-bindgen wrapper for /tools/json-remove-nulls/.
//! Field order MUST match meta.toml: json, remove_empty_strings,
//! remove_empty_arrays, remove_empty_objects, trim_strings, arrays, indent.
//! Fields arrive as strings; checkboxes as "true"/"false".
use gizza_ai_json_remove_nulls_core::{remove_nulls, Arrays, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    remove_empty_strings: &str,
    remove_empty_arrays: &str,
    remove_empty_objects: &str,
    trim_strings: &str,
    arrays: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        remove_empty_strings: truthy(remove_empty_strings),
        remove_empty_arrays: truthy(remove_empty_arrays),
        remove_empty_objects: truthy(remove_empty_objects),
        trim_strings: truthy(trim_strings),
        arrays: Arrays::parse(arrays),
        indent: indent.trim().parse().unwrap_or(2),
    };
    remove_nulls(json, opts).map_err(|e| JsValue::from_str(&e))
}
