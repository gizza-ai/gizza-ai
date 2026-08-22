//! Browser-facing wasm-bindgen wrapper for /tools/line-protocol-csv-converter/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the two boolean fields
//! here; the core owns all validation. Param order MUST match page/meta.toml's
//! [[input]] order.
use gizza_ai_line_protocol_csv_converter_core::convert;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    direction: &str,
    csv_layout: &str,
    delimiter: &str,
    timestamp_format: &str,
    precision: &str,
    emit_annotations: &str,
    measurement: &str,
    tag_columns: &str,
    field_columns: &str,
    time_column: &str,
    number_type: &str,
    sort_keys: &str,
    on_error: &str,
) -> Result<String, JsValue> {
    // sort_keys defaults true (InfluxDB's recommended tag ordering);
    // emit_annotations defaults false. Accept "true"/"1"/"on"/"yes" as truthy.
    let emit_annotations = parse_bool(emit_annotations, false);
    let sort_keys = parse_bool(sort_keys, true);
    convert(
        data,
        direction,
        csv_layout,
        delimiter,
        timestamp_format,
        precision,
        emit_annotations,
        measurement,
        tag_columns,
        field_columns,
        time_column,
        number_type,
        sort_keys,
        on_error,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
