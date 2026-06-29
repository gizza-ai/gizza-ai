//! Browser-facing wasm-bindgen wrapper for /tools/regex-tester/.
//! Field order MUST match meta.toml: text, pattern, ignore_case, multiline,
//! dotall. Fields arrive as strings.
use gizza_ai_regex_tester_core::render;
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
) -> Result<String, JsValue> {
    render(
        text,
        pattern,
        truthy(ignore_case),
        truthy(multiline),
        truthy(dotall),
    )
    .map_err(|e| JsValue::from_str(&e))
}
