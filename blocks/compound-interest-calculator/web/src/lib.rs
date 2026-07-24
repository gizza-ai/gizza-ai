//! Browser-facing wasm-bindgen wrapper for /tools/compound-interest-calculator/.
//!
//! Field order MUST match page/meta.toml: principal, annual_rate, years, months,
//! compounding, contribution, contribution_frequency, contribution_timing. Each
//! value arrives as a string; blank numeric strings are treated as "unset" (the
//! core applies the documented default), and blank enum strings fall through to
//! the core default too.
use gizza_ai_compound_interest_calculator_core::Inputs;
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

/// Compute the compound-interest result from the supplied fields, returning a
/// pretty-printed JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    principal: &str,
    annual_rate: &str,
    years: &str,
    months: &str,
    compounding: &str,
    contribution: &str,
    contribution_frequency: &str,
    contribution_timing: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        principal: parse_opt("principal", principal).map_err(js)?,
        annual_rate: parse_opt("annual_rate", annual_rate).map_err(js)?,
        years: parse_opt("years", years).map_err(js)?,
        months: parse_opt("months", months).map_err(js)?,
        compounding: opt_str(compounding),
        contribution: parse_opt("contribution", contribution).map_err(js)?,
        contribution_frequency: opt_str(contribution_frequency),
        contribution_timing: opt_str(contribution_timing),
    };
    gizza_ai_compound_interest_calculator_core::compute_json(&inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
