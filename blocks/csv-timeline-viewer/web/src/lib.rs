//! Browser-facing wasm-bindgen wrapper for /tools/csv-timeline-viewer/.
//! Field order MUST match meta.toml: data, format, delimiter, header, time_column,
//! from, to, tz_offset, search, search_fields, regex, case_sensitive, filters,
//! sort_by, order, columns, limit, offset, output.
//! Fields arrive as strings (checkboxes send "true"/"false"); numerics are f64
//! so wasm-bindgen never hands JS a BigInt.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Empty enum fields fall back to the schema default.
fn or(s: &str, fallback: &'static str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    format: &str,
    delimiter: &str,
    header: &str,
    time_column: &str,
    from: &str,
    to: &str,
    tz_offset: f64,
    search: &str,
    search_fields: &str,
    regex: &str,
    case_sensitive: &str,
    filters: &str,
    sort_by: &str,
    order: &str,
    columns: &str,
    limit: f64,
    offset: f64,
    output: &str,
) -> Result<String, JsValue> {
    let limit = if limit <= 0.0 { 100.0 } else { limit };
    let offset = if offset < 0.0 { 0.0 } else { offset };
    gizza_ai_csv_timeline_viewer_core::view(
        data,
        &or(format, "auto"),
        &or(delimiter, "auto"),
        truthy(header),
        time_column,
        from,
        to,
        tz_offset,
        search,
        search_fields,
        truthy(regex),
        truthy(case_sensitive),
        filters,
        sort_by,
        &or(order, "asc"),
        columns,
        limit.round() as u32,
        offset.round() as u32,
        &or(output, "table"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
