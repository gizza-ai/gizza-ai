//! Browser-facing wasm-bindgen wrapper for /tools/csv-row-index-adder/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_i64(s: &str, field: &str, default: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    mode: &str,
    column_name: &str,
    position: &str,
    reference_column: &str,
    has_header: &str,
    start: &str,
    step: &str,
    pad_width: &str,
    prefix: &str,
    suffix: &str,
    columns: &str,
    separator: &str,
    uuid_version: &str,
    uuid_format: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let start = parse_i64(start, "start", 1)?;
    let step = parse_i64(step, "step", 1)?;
    let pad_width = parse_i64(pad_width, "pad_width", 0)?;
    let now_ms = js_sys::Date::now() as u64;
    gizza_ai_csv_row_index_adder_core::add_index(
        data,
        if mode.is_empty() { "sequential" } else { mode },
        column_name,
        if position.is_empty() {
            "start"
        } else {
            position
        },
        reference_column,
        truthy(has_header) || has_header.trim().is_empty(),
        start,
        step,
        pad_width,
        prefix,
        suffix,
        columns,
        if separator.is_empty() { "-" } else { separator },
        if uuid_version.is_empty() {
            "4"
        } else {
            uuid_version
        },
        if uuid_format.is_empty() {
            "standard"
        } else {
            uuid_format
        },
        if delimiter.is_empty() {
            "auto"
        } else {
            delimiter
        },
        now_ms,
    )
    .map_err(|e| JsValue::from_str(&e))
}
