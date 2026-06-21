//! Browser-facing wasm-bindgen wrapper for /tools/regex-extract/.
//! Field order MUST match meta.toml: text, pattern, ignore_case, multiline,
//! dotall, capture_group, unique. Fields arrive as strings.
use gizza_ai_regex_extract_core::render;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    pattern: &str,
    ignore_case: &str,
    multiline: &str,
    dotall: &str,
    capture_group: &str,
    unique: &str,
) -> Result<String, JsValue> {
    let group: usize = capture_group.trim().parse().unwrap_or(0);
    render(
        text,
        pattern,
        truthy(ignore_case),
        truthy(multiline),
        truthy(dotall),
        group,
        truthy(unique),
    )
    .map_err(|e| JsValue::from_str(&e))
}
