//! Browser-facing wasm-bindgen wrapper for /tools/csv-window-functions/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    function: &str,
    column: &str,
    partition_by: &str,
    order_by: &str,
    window: &str,
    offset: &str,
    output_column: &str,
    descending: &str,
    has_header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let window = window.trim().parse::<i64>().unwrap_or(3);
    let offset = offset.trim().parse::<i64>().unwrap_or(1);
    let descending = truthy(descending);
    let has_header = truthy_default(has_header, true);
    gizza_ai_csv_window_functions_core::window(
        data,
        empty_default(function, "running_total"),
        column,
        partition_by,
        order_by,
        window,
        offset,
        output_column,
        descending,
        has_header,
        empty_default(delimiter, ","),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn empty_default<'a>(s: &'a str, d: &'a str) -> &'a str {
    if s.trim().is_empty() {
        d
    } else {
        s
    }
}
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
fn truthy_default(s: &str, default: bool) -> bool {
    if s.trim().is_empty() {
        default
    } else {
        truthy(s)
    }
}
