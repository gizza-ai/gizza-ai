//! Browser-facing wasm-bindgen wrapper for /tools/child-growth-percentile/.
use gizza_ai_child_growth_percentile_core::{
    decimals_from, parse_age, report, Chart, Options, Sex, Units,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    sex: &str,
    age: &str,
    height: &str,
    weight: &str,
    head_circumference: &str,
    units: &str,
    chart: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    let parse_optional = |label: &str, value: &str| -> Result<f64, JsValue> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(0.0);
        }
        trimmed
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{label} '{trimmed}' is not a number")))
    };
    let decimals = if decimals.trim().is_empty() {
        2
    } else {
        decimals.trim().parse::<i64>().map_err(|_| {
            JsValue::from_str(&format!("decimals '{}' is not an integer", decimals.trim()))
        })?
    };
    let opts = Options {
        sex: Sex::parse(sex).map_err(|e| JsValue::from_str(&e))?,
        age_months: parse_age(age).map_err(|e| JsValue::from_str(&e))?,
        height: parse_optional("height", height)?,
        weight: parse_optional("weight", weight)?,
        head_circumference: parse_optional("head_circumference", head_circumference)?,
        units: Units::parse(units).map_err(|e| JsValue::from_str(&e))?,
        chart: Chart::parse(chart).map_err(|e| JsValue::from_str(&e))?,
        decimals: decimals_from(decimals).map_err(|e| JsValue::from_str(&e))?,
    };
    report(&opts).map_err(|e| JsValue::from_str(&e))
}
