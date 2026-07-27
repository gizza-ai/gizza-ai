//! Browser-facing wasm-bindgen wrapper for /tools/enum-domain-check/.
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
    allowed: &str,
    ignore_case: &str,
    trim: &str,
    has_header: &str,
    allow_blank: &str,
    delimiter: &str,
    max_issues: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_enum_domain_check_core::run(
        data,
        column,
        allowed,
        truthy(ignore_case, false),
        truthy(trim, true),
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
