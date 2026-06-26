//! Browser-facing wasm-bindgen wrapper for /tools/list-converter/.
//! Field order MUST match meta.toml: input, input_separator, custom_input_separator, output_format, custom_output_separator, sort_mode, dedupe, case_transform, prefix, suffix, seed.
use gizza_ai_list_converter_core::{
    convert, parse_case_transform, parse_in_sep, parse_out_format, parse_sort_mode,
};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_separator: &str,
    custom_input_separator: &str,
    output_format: &str,
    custom_output_separator: &str,
    sort_mode: &str,
    dedupe: &str,
    case_transform: &str,
    prefix: &str,
    suffix: &str,
    seed: f64,
) -> Result<String, JsValue> {
    let insep = parse_in_sep(input_separator).map_err(|e| JsValue::from_str(&e))?;
    let outf = parse_out_format(output_format).map_err(|e| JsValue::from_str(&e))?;
    let smode = parse_sort_mode(sort_mode).map_err(|e| JsValue::from_str(&e))?;
    let ctrans = parse_case_transform(case_transform).map_err(|e| JsValue::from_str(&e))?;
    let is_dedupe = truthy(dedupe);

    convert(
        input,
        insep,
        custom_input_separator,
        outf,
        custom_output_separator,
        smode,
        is_dedupe,
        ctrans,
        prefix,
        suffix,
        seed as u64,
    )
    .map_err(|e| JsValue::from_str(&e))
}
