//! Browser-facing wasm-bindgen wrapper for /tools/egfr-calc/.
//!
//! Field order MUST match page/meta.toml: creatinine, creatinine_unit, age,
//! sex, equation. Each value arrives as a string; blank numeric strings are
//! treated as "unset" (the core applies the documented default), and blank enum
//! strings fall through to the core default too.
use gizza_ai_egfr_calc_core::Inputs;
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

/// Compute the eGFR result from the supplied fields, returning a pretty-printed
/// JSON object. Throws the error string on failure.
#[wasm_bindgen]
pub fn run(
    creatinine: &str,
    creatinine_unit: &str,
    age: &str,
    sex: &str,
    equation: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        creatinine: parse_opt("creatinine", creatinine).map_err(js)?,
        creatinine_unit: opt_str(creatinine_unit),
        age: parse_opt("age", age).map_err(js)?,
        sex: opt_str(sex),
        equation: opt_str(equation),
    };
    gizza_ai_egfr_calc_core::compute_json(&inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
