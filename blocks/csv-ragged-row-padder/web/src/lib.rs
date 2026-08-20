//! Browser-facing wasm-bindgen wrapper for /tools/csv-ragged-row-padder/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    width: u32,
    width_from: &str,
    long_rows: &str,
    pad_value: &str,
    header: bool,
    delimiter: &str,
    drop_empty_rows: bool,
    line_ending: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_csv_ragged_row_padder_core::pad_ragged(
        input,
        width as usize,
        width_from,
        long_rows,
        pad_value,
        header,
        delimiter,
        drop_empty_rows,
        line_ending,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
