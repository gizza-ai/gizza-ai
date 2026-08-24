//! Browser-facing wasm-bindgen wrapper for /tools/bson-extended-json-converter/.
use gizza_ai_bson_extended_json_converter_core::{convert, DateFormat, Direction, Mode, Options};
use wasm_bindgen::prelude::*;

/// The page sends every field as a string; checkboxes arrive as "true"/"false".
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    direction: &str,
    mode: &str,
    date_format: &str,
    detect_types: &str,
    big_numbers_as_strings: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let dir = Direction::parse(direction).map_err(|e| JsValue::from_str(&e))?;
    let opts = Options {
        mode: Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?,
        date_format: DateFormat::parse(date_format).map_err(|e| JsValue::from_str(&e))?,
        detect_types: truthy(detect_types),
        big_numbers_as_strings: truthy(big_numbers_as_strings),
        // `pretty` defaults ON, so an empty value must stay pretty.
        pretty: pretty.trim().is_empty() || truthy(pretty),
    };
    convert(input, dir, opts).map_err(|e| JsValue::from_str(&e))
}
