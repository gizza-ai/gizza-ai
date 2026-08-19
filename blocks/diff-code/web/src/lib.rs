//! Browser-facing wasm-bindgen wrapper for /tools/diff-code/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    view: &str,
    granularity: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    context: &str,
    line_numbers: &str,
    width: &str,
) -> Result<String, JsValue> {
    gizza_ai_diff_code_core::run(
        left,
        right,
        view,
        granularity,
        truthy(ignore_case),
        truthy(ignore_whitespace),
        context.trim().parse::<usize>().unwrap_or(3),
        truthy_default_on(line_numbers),
        width.trim().parse::<usize>().unwrap_or(60),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// `line_numbers` defaults to true, so an empty/absent value means on.
fn truthy_default_on(s: &str) -> bool {
    if s.trim().is_empty() {
        true
    } else {
        truthy(s)
    }
}
