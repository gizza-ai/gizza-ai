//! Browser-facing wasm-bindgen wrapper for /tools/query-result-formatter/.
//! Field order MUST match meta.toml: data, input_format, format, header, align, null_text.
use gizza_ai_query_result_formatter_core::{format_result, Align, Format, InputFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    input_format: &str,
    format: &str,
    header: &str,
    align: &str,
    null_text: &str,
) -> Result<String, JsValue> {
    let inf = InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Format::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let al = Align::parse(align).map_err(|e| JsValue::from_str(&e))?;
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    format_result(data, inf, fmt, hdr, al, null_text).map_err(|e| JsValue::from_str(&e))
}
