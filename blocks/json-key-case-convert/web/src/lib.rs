//! Browser-facing wasm-bindgen wrapper for /tools/json-key-case-convert/.
//! Field order MUST match meta.toml: json, target_case, recurse, preserve_keys,
//! preserve_prefix, indent. Fields arrive as strings; checkboxes as "true"/"false".
use gizza_ai_json_key_case_convert_core::{convert, parse_preserve_keys, Case, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    target_case: &str,
    recurse: &str,
    preserve_keys: &str,
    preserve_prefix: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        target: Case::parse(target_case).map_err(|e| JsValue::from_str(&e))?,
        recurse: truthy(recurse),
        preserve_keys: parse_preserve_keys(preserve_keys),
        preserve_prefix: truthy(preserve_prefix),
        indent: indent.trim().parse().unwrap_or(2),
    };
    convert(json, &opts).map_err(|e| JsValue::from_str(&e))
}
