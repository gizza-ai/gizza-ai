//! Browser-facing wasm-bindgen wrapper for /tools/z-score-calculator/.
//! Field order MUST match meta.toml: values, mode, mean, std_dev, n, sample,
//! decimals. The page marshals every field as a string, so the numeric and
//! boolean ones are parsed here.
use gizza_ai_z_score_calculator_core::summary;
use wasm_bindgen::prelude::*;

/// Parse a numeric field, falling back to `fallback` when it is left blank.
fn num(v: &str, fallback: f64, field: &str) -> Result<f64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{field} must be a number, got '{v}'"))
}

/// Parse a whole-number field with an inclusive range check.
fn int(v: &str, fallback: i64, lo: i64, hi: i64, field: &str) -> Result<i64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    let n: i64 = t
        .parse()
        .map_err(|_| format!("{field} must be a whole number, got '{v}'"))?;
    if n < lo || n > hi {
        return Err(format!("{field} must be between {lo} and {hi} (got {n})"));
    }
    Ok(n)
}

#[wasm_bindgen]
pub fn run(
    values: &str,
    mode: &str,
    mean: &str,
    std_dev: &str,
    n: &str,
    sample: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    let inner = || -> Result<String, String> {
        let mean = num(mean, 0.0, "mean")?;
        let std_dev = num(std_dev, 1.0, "standard deviation")?;
        let n = int(n, 1, 1, 1_000_000, "sample size n")?;
        let decimals = int(decimals, 6, 0, 12, "decimals")?;
        let sample = matches!(
            sample.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        );
        let mode = if mode.trim().is_empty() { "score" } else { mode };
        summary(values, mode, mean, std_dev, n as u32, sample, decimals as u32)
    };
    inner().map_err(|e| JsValue::from_str(&e))
}
