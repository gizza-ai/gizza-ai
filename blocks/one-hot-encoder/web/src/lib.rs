//! Browser-facing wasm-bindgen wrapper for /tools/one-hot-encoder/.
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
    prefix: &str,
    separator: &str,
    drop: &str,
    drop_original: &str,
    missing: &str,
    max_categories: &str,
    min_count: &str,
    other_column: &str,
    positive: &str,
    negative: &str,
    case_sensitive: &str,
    sort: &str,
    has_header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    gizza_ai_one_hot_encoder_core::encode(
        data,
        column,
        prefix,
        // The separator is meaningful whitespace-free text but "" is a legal choice
        // (cityParis); only an untouched field falls back to the default.
        if separator.is_empty() { "_" } else { separator },
        or_default(drop, "none"),
        truthy(drop_original, true),
        or_default(missing, "zeros"),
        parse_usize_default(max_categories, 0, "max_categories")?,
        parse_usize_default(min_count, 0, "min_count")?,
        truthy(other_column, false),
        or_default(positive, "1"),
        or_default(negative, "0"),
        truthy(case_sensitive, true),
        or_default(sort, "alphabetical"),
        truthy(has_header, true),
        or_default(delimiter, "comma"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
