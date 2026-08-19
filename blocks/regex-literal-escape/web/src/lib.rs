//! Browser-facing wasm-bindgen wrapper for /tools/regex-literal-escape/.
//! The standalone tool page passes every field value as a string; the two checkboxes
//! arrive as "true"/"false" and are parsed to bools here.
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
    flavor: &str,
    delimiter: &str,
    escape_whitespace: &str,
    string_literal: &str,
) -> Result<String, JsValue> {
    gizza_ai_regex_literal_escape_core::run(
        text,
        flavor,
        delimiter,
        truthy(escape_whitespace),
        truthy(string_literal),
    )
    .map_err(|e| JsValue::from_str(&e))
}
