//! Browser-facing wasm-bindgen wrapper for /tools/percent-difference-calculator/.
//!
//! Argument order MUST match the descriptor param order (a, b, mode, decimals),
//! which is also the page/meta.toml field order. Every field arrives as a
//! string; `mode` and `decimals` fall back to their descriptor defaults when
//! blank, and a blank value field means "nothing entered yet" rather than an
//! error.
use gizza_ai_percent_difference_calculator_core::{summary, MAX_DECIMALS};
use wasm_bindgen::prelude::*;

/// Parse one required numeric field.
fn parse_value(label: &str, s: &str) -> Result<f64, String> {
    let t = s.trim();
    t.parse::<f64>()
        .map_err(|_| format!("{label} must be a number (got '{t}')"))
}

/// Compare two values and return the human-readable report. Throws the error
/// string on failure.
#[wasm_bindgen]
pub fn run(a: &str, b: &str, mode: &str, decimals: &str) -> Result<String, JsValue> {
    // The mode <select> always has a value, so an untouched form can't be
    // detected the way single-field tools do — prompt instead of erroring.
    if a.trim().is_empty() || b.trim().is_empty() {
        return Ok("Enter both values to compare.".to_string());
    }

    let a = parse_value("a", a).map_err(js)?;
    let b = parse_value("b", b).map_err(js)?;
    let mode = if mode.trim().is_empty() { "all" } else { mode };
    let decimals = match decimals.trim() {
        "" => 4,
        d => d
            .parse::<u32>()
            .map_err(|_| js(format!("decimals must be a whole number 0 to {MAX_DECIMALS}")))?,
    };
    summary(a, b, mode, decimals).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
