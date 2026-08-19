//! Browser-facing wasm-bindgen wrapper for /tools/arff-converter/.
use gizza_ai_arff_converter_core::{convert, ArffFormat, Direction, Options};
use wasm_bindgen::prelude::*;

fn truthy_default_on(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

fn truthy_default_off(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_i64(v: &str, fallback: i64, field: &str) -> Result<i64, JsValue> {
    if v.trim().is_empty() {
        return Ok(fallback);
    }
    v.trim()
        .parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    direction: &str,
    delimiter: &str,
    header: &str,
    relation: &str,
    nominal_threshold: &str,
    column_types: &str,
    date_format: &str,
    missing_value: &str,
    arff_format: &str,
    type_row: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        direction: Direction::parse(direction).map_err(|e| JsValue::from_str(&e))?,
        delimiter: if delimiter.is_empty() {
            ",".to_string()
        } else {
            delimiter.to_string()
        },
        header: truthy_default_on(header),
        relation: relation.to_string(),
        nominal_threshold: parse_i64(nominal_threshold, 10, "nominal_threshold")?,
        column_types: column_types.to_string(),
        date_format: date_format.to_string(),
        missing_value: missing_value.to_string(),
        arff_format: ArffFormat::parse(arff_format).map_err(|e| JsValue::from_str(&e))?,
        type_row: truthy_default_off(type_row),
    };
    convert(data, &opts).map_err(|e| JsValue::from_str(&e))
}
