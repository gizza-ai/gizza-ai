//! Browser-facing wasm-bindgen wrapper for /tools/frequency-encoder/.
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
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number ≥ 0")))
    }
}

fn or_default<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    column: &str,
    mode: &str,
    output: &str,
    blank: &str,
    min_count: &str,
    case_sensitive: &str,
    decimals: &str,
    has_header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    gizza_ai_frequency_encoder_core::encode(
        data,
        column,
        or_default(mode, "count"),
        or_default(output, "replace"),
        or_default(blank, "count"),
        parse_usize_default(min_count, 0, "min_count")?,
        truthy(case_sensitive, true),
        parse_usize_default(decimals, 4, "decimals")?,
        truthy(has_header, true),
        or_default(delimiter, "comma"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
