//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-xlsx/.
//!
//! Parses the pasted table and returns the workbook as a `data:` URL string;
//! the page's custom.js turns that into a Download button. Every argument
//! arrives from the form as a string (the page driver passes field values as
//! strings), so booleans are parsed positive-truthy here.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_csv_to_xlsx_core::{to_xlsx, InputFormat};
use wasm_bindgen::prelude::*;

const XLSX_MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// Parse a checkbox/string value as a boolean. An empty value falls back to
/// `default` (the form only sends empties before a checkbox has rendered).
fn truthy(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    input_format: &str,
    sheet_name: &str,
    header: &str,
    detect_types: &str,
    autofit: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page renders a neutral idle state rather
    // than a red "input is empty" error on first load / after Reset).
    if data.trim().is_empty() {
        return Ok(String::new());
    }
    let fmt = InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?;
    let sheet = if sheet_name.trim().is_empty() { "Sheet1" } else { sheet_name };
    let bytes = to_xlsx(
        data,
        fmt,
        sheet,
        truthy(header, true),
        truthy(detect_types, true),
        truthy(autofit, true),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("data:{XLSX_MIME};base64,{}", B64.encode(&bytes)))
}
