//! Browser-facing wasm-bindgen wrapper for /tools/html-diff/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    left: &str,
    right: &str,
    granularity: &str,
    format: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    context: &str,
) -> Result<String, JsValue> {
    let ignore_case = truthy(ignore_case);
    let ignore_whitespace = truthy(ignore_whitespace);
    let context = context.trim().parse::<usize>().unwrap_or(3);
    gizza_ai_html_diff_core::run(
        left,
        right,
        granularity,
        format,
        ignore_case,
        ignore_whitespace,
        context,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
