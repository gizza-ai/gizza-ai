//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-pdf-table/.
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        default
    } else {
        !matches!(t.as_str(), "false" | "0" | "off" | "no")
    }
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    header: &str,
    title: &str,
    page_size: &str,
    orientation: &str,
    font_size: &str,
    row_banding: &str,
    grid: &str,
) -> Result<String, JsValue> {
    let font_size = if font_size.trim().is_empty() {
        10.0
    } else {
        font_size
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str("font_size must be a number from 5 to 24"))?
    };
    let pdf = gizza_ai_csv_to_pdf_table_core::render_csv_pdf(
        data,
        if delimiter.trim().is_empty() { "comma" } else { delimiter },
        parse_bool(header, true),
        title,
        if page_size.trim().is_empty() { "letter" } else { page_size },
        if orientation.trim().is_empty() { "portrait" } else { orientation },
        font_size,
        parse_bool(row_banding, true),
        parse_bool(grid, true),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "data:application/pdf;base64,{}",
        B64.encode(pdf)
    ))
}
