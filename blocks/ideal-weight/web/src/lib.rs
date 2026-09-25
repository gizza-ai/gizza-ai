//! Browser-facing wasm-bindgen wrapper for /tools/ideal-weight/.
//!
//! Field order MUST match page/meta.toml: height, sex, units, frame, wrist, age,
//! bmi_min, bmi_max. Each value arrives as a string; blank numeric strings are
//! treated as "unset" (the core applies the documented default, and an unset
//! wrist stays `None` so frame=auto can report what it needed), and blank enum
//! strings fall through to the core default too.
use gizza_ai_ideal_weight_core::Inputs;
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

/// Compute the ideal-weight report from the supplied fields, returning a
/// pretty-printed JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    height: &str,
    sex: &str,
    units: &str,
    frame: &str,
    wrist: &str,
    age: &str,
    bmi_min: &str,
    bmi_max: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        height: parse_opt("height", height).map_err(js)?,
        sex: opt_str(sex),
        units: opt_str(units),
        frame: opt_str(frame),
        wrist: parse_opt("wrist", wrist).map_err(js)?,
        age: parse_opt("age", age).map_err(js)?,
        bmi_min: parse_opt("bmi_min", bmi_min).map_err(js)?,
        bmi_max: parse_opt("bmi_max", bmi_max).map_err(js)?,
    };
    gizza_ai_ideal_weight_core::compute_json(&inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
