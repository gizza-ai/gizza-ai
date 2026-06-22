//! Browser-facing wasm-bindgen wrapper for /tools/list-converter/.
//! Field order MUST match meta.toml: input, input_separator, output_format, sort, dedupe.
use gizza_ai_list_converter_core::{convert, parse_in_sep, parse_out_format};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(input: &str, input_separator: &str, output_format: &str, sort: &str, dedupe: &str) -> Result<String, JsValue> {
    let insep = parse_in_sep(input_separator).map_err(|e| JsValue::from_str(&e))?;
    let outf = parse_out_format(output_format).map_err(|e| JsValue::from_str(&e))?;
    convert(input, insep, outf, truthy(sort), truthy(dedupe)).map_err(|e| JsValue::from_str(&e))
}
