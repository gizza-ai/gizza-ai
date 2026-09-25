//! Browser-facing wasm-bindgen wrapper for /tools/html-accessibility-checker/.
//! Field order MUST match page/meta.toml: html, level, min_severity, format,
//! show_passed, max_issues.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    html: &str,
    level: &str,
    min_severity: &str,
    format: &str,
    show_passed: &str,
    max_issues: f64,
) -> Result<String, JsValue> {
    let fmt = gizza_ai_html_accessibility_checker_core::parse_format(format)
        .map_err(|e| JsValue::from_str(&e))?;
    let opts = gizza_ai_html_accessibility_checker_core::Options {
        level: gizza_ai_html_accessibility_checker_core::parse_level(level)
            .map_err(|e| JsValue::from_str(&e))?,
        min_severity: gizza_ai_html_accessibility_checker_core::parse_severity(min_severity)
            .map_err(|e| JsValue::from_str(&e))?,
        show_passed: truthy(show_passed),
        max_issues: if max_issues.is_finite() && max_issues >= 1.0 {
            max_issues.round() as usize
        } else {
            200
        },
    };
    gizza_ai_html_accessibility_checker_core::check_to_string(html, fmt, &opts)
        .map_err(|e| JsValue::from_str(&e))
}
