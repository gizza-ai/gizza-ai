//! Browser-facing wasm-bindgen wrapper for /tools/csv-column-split/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    mode: &str,
    columns: &str,
    separator: &str,
    names: &str,
    max_columns: &str,
    keep_source: &str,
    trim: &str,
    has_header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let max_columns = max_columns.trim().parse::<usize>().unwrap_or(0);
    let keep_source = truthy(keep_source);
    let trim = truthy_default(trim, true);
    let has_header = truthy_default(has_header, true);
    gizza_ai_csv_column_split_core::column_split(
        data,
        empty_default(mode, "split"),
        columns,
        empty_default(separator, ","),
        names,
        max_columns,
        keep_source,
        trim,
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
