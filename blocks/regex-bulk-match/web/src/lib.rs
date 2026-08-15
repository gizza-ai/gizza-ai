//! Browser-facing wasm-bindgen wrapper for /tools/regex-bulk-match/.
//! Field order MUST match page/meta.toml. Fields arrive as strings from the
//! generic form runtime.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_max_lines(v: &str) -> usize {
    v.trim().parse::<usize>().unwrap_or(1000)
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    lines: &str,
    pattern: &str,
    show: &str,
    output: &str,
    max_lines: &str,
    full_match: &str,
    ignore_case: &str,
    dotall: &str,
    trim: &str,
    skip_blank: &str,
    captures: &str,
    show_position: &str,
) -> Result<String, JsValue> {
    gizza_ai_regex_bulk_match_core::run(
        lines,
        pattern,
        truthy(full_match),
        truthy(ignore_case),
        truthy(dotall),
        truthy(trim),
        truthy(skip_blank),
        truthy(captures),
        truthy(show_position),
        show,
        parse_max_lines(max_lines),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
