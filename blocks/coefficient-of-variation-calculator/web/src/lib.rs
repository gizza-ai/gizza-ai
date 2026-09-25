//! Browser-facing wasm-bindgen wrapper for /tools/coefficient-of-variation-calculator/.
//! Compiled with wasm-pack for the standalone page. Field order MUST match the
//! `[[input]]` order in page/meta.toml: data, basis, grouping, delimiter, mean,
//! std_dev, exclude_outliers, ignore_non_numeric, decimals, output.
//!
//! Numeric page fields arrive as strings in the generic pure-tool runtime. Parse
//! optional summary numbers here so a blank box stays "not supplied" rather than
//! collapsing to 0 — the core needs that distinction to tell summary mode apart
//! from a genuine zero mean. The core owns all statistical validation.
use wasm_bindgen::prelude::*;

fn opt_number(raw: &str, name: &str) -> Result<Option<f64>, JsValue> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let n = trimmed
        .parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))?;
    if n.is_finite() {
        Ok(Some(n))
    } else {
        Err(JsValue::from_str(&format!("{name} must be finite")))
    }
}

fn number_or(raw: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    Ok(opt_number(raw, name)?.unwrap_or(default))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    basis: &str,
    grouping: &str,
    delimiter: &str,
    mean: &str,
    std_dev: &str,
    exclude_outliers: bool,
    ignore_non_numeric: bool,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_coefficient_of_variation_calculator_core::run(
        data,
        basis,
        grouping,
        delimiter,
        opt_number(mean, "mean")?,
        opt_number(std_dev, "std_dev")?,
        exclude_outliers,
        ignore_non_numeric,
        number_or(decimals, 4.0, "decimals")?,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
