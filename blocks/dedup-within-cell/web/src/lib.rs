//! Browser-facing wasm-bindgen wrapper for /tools/dedup-within-cell/.
use wasm_bindgen::prelude::*;

fn boolish(s: &str, default: bool) -> bool {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        return default;
    }
    matches!(t.as_str(), "true" | "1" | "on" | "yes")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    item_separator: &str,
    output_separator: &str,
    ignore_case: &str,
    trim_items: &str,
    drop_empty: &str,
    sort_items: &str,
    delimiter: &str,
    has_header: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_dedup_within_cell_core::dedupe_within_cells(
        data,
        columns,
        if item_separator.trim().is_empty() {
            "comma"
        } else {
            item_separator
        },
        output_separator,
        boolish(ignore_case, false),
        boolish(trim_items, true),
        boolish(drop_empty, true),
        if sort_items.trim().is_empty() {
            "none"
        } else {
            sort_items
        },
        if delimiter.trim().is_empty() {
            "comma"
        } else {
            delimiter
        },
        boolish(has_header, true),
        if output.trim().is_empty() {
            "csv"
        } else {
            output
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
