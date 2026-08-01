//! Browser-facing wasm-bindgen wrapper for /tools/csv-fill-down/.
use gizza_ai_csv_fill_down_core::fill_down;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    direction: &str,
    header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let hdr = !matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let dir = if direction.is_empty() { "down" } else { direction };
    fill_down(data, columns, dir, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
