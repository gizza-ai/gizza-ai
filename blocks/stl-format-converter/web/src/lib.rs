//! Browser-facing wasm-bindgen wrapper for /tools/stl-format-converter/.
//! Compiled with wasm-pack for the standalone /tools/stl-format-converter/ page.
//!
//! Field order MUST match page/meta.toml: stl, input_format, to, output,
//! solid_name, normals, number_format, precision, output_encoding. The page
//! passes every field value as a string, so blanks fall back to the descriptor
//! defaults here.
use gizza_ai_stl_format_converter_core::{
    convert, InputFormat, NumberFormat, Normals, Options, Output, OutputEncoding, Target,
};
use wasm_bindgen::prelude::*;

/// Parse an integer field, treating a blank/unparseable value as `fallback`.
fn int(s: &str, fallback: u32) -> u32 {
    let t = s.trim();
    if t.is_empty() {
        fallback
    } else {
        t.parse::<f64>().map(|v| v.round() as u32).unwrap_or(fallback)
    }
}

/// Convert an STL between binary and ASCII encodings.
///
/// - `stl`: ASCII STL text, or binary STL bytes as base64/hex.
/// - `input_format`: `"auto"` (default), `"ascii"`, `"base64"` or `"hex"`.
/// - `to`: `"auto"` (default, flip), `"ascii"` or `"binary"`.
/// - `output`: `"stl"` (default) or `"summary"`.
/// - `solid_name`: name for the ASCII solid / binary header (blank keeps the source's).
/// - `normals`: `"keep"` (default), `"recompute"` or `"zero"`.
/// - `number_format`: `"scientific"` (default) or `"decimal"`.
/// - `precision`: ASCII decimal places, 0-17 (blank → 6).
/// - `output_encoding`: `"data-url"` (default), `"base64"` or `"hex"`.
///
/// Throws a JS error string on invalid arguments or an unparseable mesh.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    stl: &str,
    input_format: &str,
    to: &str,
    output: &str,
    solid_name: &str,
    normals: &str,
    number_format: &str,
    precision: &str,
    output_encoding: &str,
) -> Result<String, JsValue> {
    let opt = Options {
        input_format: InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?,
        to: Target::parse(to).map_err(|e| JsValue::from_str(&e))?,
        output_encoding: OutputEncoding::parse(output_encoding)
            .map_err(|e| JsValue::from_str(&e))?,
        solid_name: solid_name.to_string(),
        normals: Normals::parse(normals).map_err(|e| JsValue::from_str(&e))?,
        precision: int(precision, 6),
        number_format: NumberFormat::parse(number_format).map_err(|e| JsValue::from_str(&e))?,
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
    };
    convert(stl, &opt).map_err(|e| JsValue::from_str(&e))
}
