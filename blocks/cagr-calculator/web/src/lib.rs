//! Browser-facing wasm-bindgen wrapper for /tools/cagr-calculator/.
//! Field order MUST match meta.toml: values, period, years, start_date,
//! end_date, target_value, has_header, decimals, output. The page marshals
//! every field as a string, so the numeric/boolean ones are parsed here.
use gizza_ai_cagr_calculator_core::summary;
use wasm_bindgen::prelude::*;

/// Parse an optional numeric field; blank means "unset" (0).
fn parse_opt_num(v: &str, field: &str) -> Result<f64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(0.0);
    }
    gizza_ai_cagr_calculator_core::parse_number(t)
        .ok_or_else(|| format!("{field} must be a number, got '{v}'"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    values: &str,
    period: &str,
    years: &str,
    start_date: &str,
    end_date: &str,
    target_value: &str,
    has_header: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    let years = parse_opt_num(years, "years").map_err(|e| JsValue::from_str(&e))?;
    let target = parse_opt_num(target_value, "target value").map_err(|e| JsValue::from_str(&e))?;
    let dp: i64 = {
        let t = decimals.trim();
        if t.is_empty() {
            2
        } else {
            t.parse()
                .map_err(|_| JsValue::from_str(&format!("decimals must be a whole number, got '{decimals}'")))?
        }
    };
    let header = matches!(
        has_header.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let period = if period.trim().is_empty() { "annual" } else { period };
    let output = if output.trim().is_empty() { "summary" } else { output };
    summary(
        values, period, years, start_date, end_date, target, header, dp, output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
