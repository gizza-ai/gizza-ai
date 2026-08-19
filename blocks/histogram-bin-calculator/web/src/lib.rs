//! Browser-facing wasm-bindgen wrapper for /tools/histogram-bin-calculator/.
//! Numeric params are `f64` (wasm-bindgen rejects i64/BigInt) and checkboxes
//! arrive as the strings "true"/"false".
use wasm_bindgen::prelude::*;

/// Page checkboxes send "true"/"false"; be liberal about the other truthy forms.
fn flag(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// A whole-number field, tolerating a blank box (falls back to `default`).
fn whole(v: f64, default: u32, name: &str, max: u32) -> Result<u32, String> {
    if !v.is_finite() || v == 0.0 && name == "precision" && default == 0 {
        return Ok(default);
    }
    if !v.is_finite() {
        return Ok(default);
    }
    if v < 0.0 || v.fract() != 0.0 {
        return Err(format!("{name} must be a whole number (got {v})"));
    }
    let n = v as u32;
    if n > max {
        return Err(format!("{name} must be at most {max} (got {n})"));
    }
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    numbers: &str,
    rule: &str,
    bins: f64,
    range_min: &str,
    range_max: &str,
    nice_edges: &str,
    right_closed: &str,
    precision: f64,
    output: &str,
    cumulative: &str,
    density: &str,
    chart: &str,
) -> Result<String, JsValue> {
    let bins = whole(bins, 10, "bins", 1000).map_err(|e| JsValue::from_str(&e))?;
    let precision = whole(precision, 4, "precision", 12).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_histogram_bin_calculator_core::run(
        numbers,
        if rule.trim().is_empty() { "auto" } else { rule },
        bins,
        range_min,
        range_max,
        flag(nice_edges),
        flag(right_closed),
        precision,
        if output.trim().is_empty() {
            "report"
        } else {
            output
        },
        flag(cumulative),
        flag(density),
        flag(chart),
    )
    .map_err(|e| JsValue::from_str(&e))
}
