//! Browser-facing wasm-bindgen wrapper for /tools/regex-to-json/.
//! Field order MUST match page/meta.toml: text, pattern, ignore_case,
//! all_matches, unmatched, coerce_types, output. Fields arrive as strings.
use gizza_ai_regex_to_json_core::to_json;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    pattern: &str,
    ignore_case: &str,
    all_matches: &str,
    unmatched: &str,
    coerce_types: &str,
    output: &str,
) -> Result<String, JsValue> {
    to_json(
        text,
        pattern,
        truthy(ignore_case),
        truthy(all_matches),
        unmatched,
        truthy(coerce_types),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
