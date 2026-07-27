//! Browser-facing wasm-bindgen wrapper for /tools/regex-column-validate/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

fn parse_usize_default(v: &str, default: usize, name: &str) -> Result<usize, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be an integer")))
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    column: &str,
    pattern: &str,
    full_match: &str,
    ignore_case: &str,
    report: &str,
    has_header: &str,
    allow_blank: &str,
    delimiter: &str,
    max_issues: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_regex_column_validate_core::run(
        data,
        column,
        pattern,
        truthy(full_match, true),
        truthy(ignore_case, false),
        if report.trim().is_empty() {
            "non-matching"
        } else {
            report
        },
        truthy(has_header, true),
        truthy(allow_blank, true),
        if delimiter.trim().is_empty() {
            "auto"
        } else {
            delimiter
        },
        parse_usize_default(max_issues, 50, "max_issues")?,
        if output.trim().is_empty() {
            "text"
        } else {
            output
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
