//! Browser-facing wasm-bindgen wrapper for /tools/qr-batch/.
//!
//! Returns the generated ZIP as a `data:application/zip;base64,...` URL. The
//! page custom.js renders that as a download button and decodes index.csv for a
//! real-output preview.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_qr_batch_core::{Columns, Ecc, InputFormat, Options, OutFormat};
use wasm_bindgen::prelude::*;

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
    columns: &str,
    has_header: &str,
    format: &str,
    size: &str,
    margin: &str,
    error_correction: &str,
    fg_color: &str,
    bg_color: &str,
    name_prefix: &str,
    include_index: &str,
) -> Result<String, JsValue> {
    if data.trim().is_empty() {
        return Ok(String::new());
    }
    let size = if size.trim().is_empty() {
        512
    } else {
        size.trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("size must be an integer"))?
    };
    let margin = if margin.trim().is_empty() {
        4
    } else {
        margin
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("margin must be an integer"))?
    };
    let opts = Options {
        input_format: InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?,
        columns: Columns::parse(columns).map_err(|e| JsValue::from_str(&e))?,
        has_header: truthy(has_header, false),
        format: OutFormat::parse(format).map_err(|e| JsValue::from_str(&e))?,
        size,
        margin,
        ecc: Ecc::parse(error_correction).map_err(|e| JsValue::from_str(&e))?,
        fg_color: if fg_color.trim().is_empty() {
            "#000000".to_string()
        } else {
            fg_color.to_string()
        },
        bg_color: if bg_color.trim().is_empty() {
            "#ffffff".to_string()
        } else {
            bg_color.to_string()
        },
        name_prefix: if name_prefix.trim().is_empty() {
            "qr".to_string()
        } else {
            name_prefix.to_string()
        },
        include_index: truthy(include_index, true),
    };
    let batch =
        gizza_ai_qr_batch_core::generate_batch(data, &opts).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "data:application/zip;base64,{}",
        B64.encode(&batch.zip)
    ))
}
