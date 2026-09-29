//! Browser-facing wasm-bindgen wrapper for /tools/barcode-batch/.
//!
//! Returns the generated bundle as a `data:application/zip;base64,…` or
//! `data:application/pdf;base64,…` URL. The page custom.js renders that as a
//! download button and decodes index.csv (ZIP) for a real-output preview.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_barcode_batch_core::{
    Columns, InputFormat, OutFormat, Options, Output, SheetPreset, Symbology,
};
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn num(v: &str, default: u32, what: &str) -> Result<u32, JsValue> {
    if v.trim().is_empty() {
        return Ok(default);
    }
    v.trim()
        .parse::<u32>()
        .map_err(|_| JsValue::from_str(&format!("{what} must be a whole number")))
}

fn or_default(v: &str, default: &str) -> String {
    if v.trim().is_empty() {
        default.to_string()
    } else {
        v.to_string()
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    input_format: &str,
    columns: &str,
    has_header: &str,
    symbology: &str,
    auto_check_digit: &str,
    output: &str,
    format: &str,
    sheet_preset: &str,
    module_width: &str,
    bar_height: &str,
    quiet_zone: &str,
    show_text: &str,
    text_size: &str,
    fg_color: &str,
    bg_color: &str,
    name_prefix: &str,
    include_index: &str,
) -> Result<String, JsValue> {
    if data.trim().is_empty() {
        return Ok(String::new());
    }
    let opts = Options {
        input_format: InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?,
        columns: Columns::parse(columns).map_err(|e| JsValue::from_str(&e))?,
        has_header: truthy(has_header, false),
        symbology: Symbology::parse(symbology).map_err(|e| JsValue::from_str(&e))?,
        auto_check_digit: truthy(auto_check_digit, true),
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
        format: OutFormat::parse(format).map_err(|e| JsValue::from_str(&e))?,
        sheet_preset: SheetPreset::parse(sheet_preset).map_err(|e| JsValue::from_str(&e))?,
        module_width: num(module_width, 2, "Bar width")?,
        bar_height: num(bar_height, 100, "Bar height")?,
        quiet_zone: num(quiet_zone, 10, "Quiet zone")?,
        show_text: truthy(show_text, true),
        text_size: num(text_size, 20, "Text size")?,
        fg_color: or_default(fg_color, "#000000"),
        bg_color: or_default(bg_color, "#ffffff"),
        name_prefix: or_default(name_prefix, "barcode"),
        include_index: truthy(include_index, true),
    };
    let batch = gizza_ai_barcode_batch_core::generate_batch(data, &opts)
        .map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "data:{};base64,{}",
        batch.mime,
        B64.encode(&batch.bytes)
    ))
}
