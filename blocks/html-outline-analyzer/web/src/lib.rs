//! Browser-facing wasm-bindgen wrapper for /tools/html-outline-analyzer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    html: &str,
    format: &str,
    min_level: &str,
    max_level: &str,
    include_issues: &str,
    include_counts: &str,
    top_tags: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    };
    let opts = gizza_ai_html_outline_analyzer_core::Options {
        min_level: min_level.trim().parse::<u8>().unwrap_or(1),
        max_level: max_level.trim().parse::<u8>().unwrap_or(6),
        include_issues: truthy(include_issues),
        include_counts: truthy(include_counts),
        top_tags: top_tags.trim().parse::<usize>().unwrap_or(20),
    };
    let fmt = gizza_ai_html_outline_analyzer_core::parse_format(format)
        .map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_html_outline_analyzer_core::analyze_to_string(html, fmt, &opts)
        .map_err(|e| JsValue::from_str(&e))
}
