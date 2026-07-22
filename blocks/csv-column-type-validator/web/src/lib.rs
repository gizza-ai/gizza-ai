//! Browser-facing wasm-bindgen wrapper for /tools/csv-column-type-validator/.
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
    types: &str,
    header: &str,
    delimiter: &str,
    date_format: &str,
    empty_ok: &str,
    max_issues: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_column_type_validator_core::run(
        data,
        types,
        flag(header, true),
        delimiter,
        date_format,
        flag(empty_ok, true),
        parse_max_issues(max_issues),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
