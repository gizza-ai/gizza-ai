//! Browser-facing wasm-bindgen wrapper for /tools/date-column-validate/.
//! Argument order matches the page's field order (meta.toml) and the core::run
//! signature; the page passes every input as a string, so booleans and the
//! issue cap are parsed here.
use wasm_bindgen::prelude::*;

fn flag(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}

fn parse_max_issues(s: &str) -> usize {
    s.trim().parse::<usize>().unwrap_or(50)
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    column: &str,
    preset: &str,
    format: &str,
    has_header: &str,
    allow_blank: &str,
    delimiter: &str,
    max_issues: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_date_column_validate_core::run(
        data,
        column,
        preset,
        format,
        flag(has_header, true),
        flag(allow_blank, true),
        delimiter,
        parse_max_issues(max_issues),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
