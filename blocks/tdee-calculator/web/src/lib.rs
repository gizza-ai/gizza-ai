//! Browser-facing wasm-bindgen wrapper for /tools/tdee-calculator/.
//!
//! Field order MUST match page/meta.toml: age, sex, weight, height, units,
//! activity, formula, body_fat, energy_unit. Each value arrives as a string;
//! blank numeric strings are treated as "unset" (the core applies the documented
//! default), and blank enum strings fall through to the core default too.
use gizza_ai_tdee_calculator_core::Inputs;
use wasm_bindgen::prelude::*;

/// Parse an optional numeric field: blank/whitespace → `None`; otherwise parse,
/// erroring on garbage.
fn parse_opt(label: &str, s: &str) -> Result<Option<f64>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("{label} must be a number (got '{t}')"))
}

/// Optional string field: blank → `None` (core supplies the default).
fn opt_str(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Compute the BMR/TDEE result from the supplied fields, returning a
/// pretty-printed JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    age: &str,
    sex: &str,
    weight: &str,
    height: &str,
    units: &str,
    activity: &str,
    formula: &str,
    body_fat: &str,
    energy_unit: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        age: parse_opt("age", age).map_err(js)?,
        sex: opt_str(sex),
        weight: parse_opt("weight", weight).map_err(js)?,
        height: parse_opt("height", height).map_err(js)?,
        units: opt_str(units),
        activity: opt_str(activity),
        formula: opt_str(formula),
        body_fat: parse_opt("body_fat", body_fat).map_err(js)?,
        energy_unit: opt_str(energy_unit),
    };
    gizza_ai_tdee_calculator_core::compute_json(&inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
