//! Browser-facing wasm-bindgen wrapper for /tools/scientific-calculator/.
use wasm_bindgen::prelude::*;

fn default_text(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

fn flag(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_usize(s: &str, fallback: usize, name: &str) -> Result<usize, JsValue> {
    let t = s.trim().replace([',', '_'], "");
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<usize>().map_err(|_| {
        JsValue::from_str(&format!(
            "{name} must be a whole number, got `{}`",
            s.trim()
        ))
    })
}

#[wasm_bindgen]
pub fn run(
    expression: &str,
    variables: &str,
    angle_unit: &str,
    precision: &str,
    notation: &str,
    complex_form: &str,
    group_digits: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    let angle_unit = default_text(angle_unit, "radians");
    let notation = default_text(notation, "auto");
    let complex_form = default_text(complex_form, "rectangular");
    let output_format = default_text(output_format, "text");
    let opts = gizza_ai_scientific_calculator_core::Options {
        expression: expression.to_string(),
        variables: variables.to_string(),
        angle_unit: gizza_ai_scientific_calculator_core::AngleUnit::parse(&angle_unit)
            .map_err(|e| JsValue::from_str(&e))?,
        precision: parse_usize(precision, 12, "precision")?,
        notation: gizza_ai_scientific_calculator_core::Notation::parse(&notation)
            .map_err(|e| JsValue::from_str(&e))?,
        complex_form: gizza_ai_scientific_calculator_core::ComplexForm::parse(&complex_form)
            .map_err(|e| JsValue::from_str(&e))?,
        group_digits: flag(group_digits, false),
        output_format: gizza_ai_scientific_calculator_core::OutputFormat::parse(&output_format)
            .map_err(|e| JsValue::from_str(&e))?,
    };
    gizza_ai_scientific_calculator_core::evaluate(opts).map_err(|e| JsValue::from_str(&e))
}
