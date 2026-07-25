//! Browser-facing wasm-bindgen wrapper for /tools/data-validator/.
//! Field order MUST match meta.toml: data, rules, input_format, header,
//! delimiter, max_issues, format. Fields arrive as strings (checkboxes send
//! "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_int(s: &str, field: &str, default: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    rules: &str,
    input_format: &str,
    header: &str,
    delimiter: &str,
    max_issues: &str,
    format: &str,
) -> Result<String, JsValue> {
    // Empty select/format fields fall back to the schema defaults.
    let fmt = if input_format.trim().is_empty() {
        "auto"
    } else {
        input_format
    };
    let delim = if delimiter.trim().is_empty() {
        "auto"
    } else {
        delimiter
    };
    let out_fmt = if format.trim().is_empty() {
        "text"
    } else {
        format
    };
    // An empty header field means the checkbox wasn't sent; default true.
    let header_on = if header.trim().is_empty() {
        true
    } else {
        truthy(header)
    };
    let max = parse_int(max_issues, "max_issues", 50)?.clamp(1, 1000) as usize;
    gizza_ai_data_validator_core::run(data, rules, fmt, header_on, delim, max, out_fmt)
        .map_err(|e| JsValue::from_str(&e))
}
