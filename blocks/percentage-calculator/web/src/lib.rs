//! Browser-facing wasm-bindgen wrapper for /tools/percentage-calculator/.
//!
//! Field order MUST match page/meta.toml: mode, then the numeric fields. Each
//! number arrives as a string; blank strings are treated as "unset" (the chosen
//! mode only reads the numbers it needs). A non-blank, non-numeric field is a
//! parse error.
use gizza_ai_percentage_calculator_core::Inputs;
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

/// Compute a percentage result for `mode` from the supplied numeric fields,
/// returning a pretty-printed JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    mode: &str,
    percent: &str,
    base: &str,
    part: &str,
    whole: &str,
    from: &str,
    to: &str,
    value: &str,
    total: &str,
) -> Result<String, JsValue> {
    // The mode <select> always has a value, so the page can't detect an "empty"
    // form the way single-field tools do. When no number has been entered yet,
    // show a prompt instead of a missing-input error.
    if [percent, base, part, whole, from, to, value, total]
        .iter()
        .all(|s| s.trim().is_empty())
    {
        return Ok(format!("Enter the numbers for {mode} to see the result."));
    }

    let inputs = Inputs {
        percent: parse_opt("percent", percent).map_err(js)?,
        base: parse_opt("base", base).map_err(js)?,
        part: parse_opt("part", part).map_err(js)?,
        whole: parse_opt("whole", whole).map_err(js)?,
        from: parse_opt("from", from).map_err(js)?,
        to: parse_opt("to", to).map_err(js)?,
        value: parse_opt("value", value).map_err(js)?,
        total: parse_opt("total", total).map_err(js)?,
    };
    gizza_ai_percentage_calculator_core::compute_json(mode, &inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
