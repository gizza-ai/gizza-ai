//! Browser-facing wasm-bindgen wrapper for /tools/keyvalue-text-parser/.
//! The page marshals each field as a string, so checkbox strings are parsed here
//! before the pure core receives typed booleans.
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
    input: &str,
    separator: &str,
    custom_separator: &str,
    structure: &str,
    duplicates: &str,
    trim: &str,
    unquote: &str,
    comment_prefixes: &str,
    infer_types: &str,
    key_case: &str,
    unmatched: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let indent = indent.trim().parse::<f64>().unwrap_or(2.0);
    gizza_ai_keyvalue_text_parser_core::parse_text(
        input,
        if separator.trim().is_empty() {
            "auto"
        } else {
            separator
        },
        custom_separator,
        if structure.trim().is_empty() {
            "object"
        } else {
            structure
        },
        if duplicates.trim().is_empty() {
            "group"
        } else {
            duplicates
        },
        truthy(trim),
        truthy(unquote),
        comment_prefixes,
        truthy(infer_types),
        if key_case.trim().is_empty() {
            "as-is"
        } else {
            key_case
        },
        if unmatched.trim().is_empty() {
            "skip"
        } else {
            unmatched
        },
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}
