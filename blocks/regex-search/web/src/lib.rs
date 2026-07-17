//! Browser-facing wasm-bindgen wrapper for /tools/regex-search/.
//! Field order MUST match meta.toml: text, pattern, literal, ignore_case,
//! whole_word, invert, context, line_numbers, highlight. Fields arrive as strings.
use gizza_ai_regex_search_core::render;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    pattern: &str,
    literal: &str,
    ignore_case: &str,
    whole_word: &str,
    invert: &str,
    context: &str,
    line_numbers: &str,
    highlight: &str,
) -> Result<String, JsValue> {
    let ctx: usize = context.trim().parse().unwrap_or(0);
    render(
        text,
        pattern,
        truthy(literal),
        truthy(ignore_case),
        truthy(whole_word),
        truthy(invert),
        ctx,
        truthy(line_numbers),
        truthy(highlight),
    )
    .map_err(|e| JsValue::from_str(&e))
}
