//! Browser-facing wasm-bindgen wrapper for /tools/time-series-forecaster/.
//! The page passes every field as a raw string (no coercion for pure tools),
//! so numeric params arrive as &str and are parsed here (blank → default).
use gizza_ai_time_series_forecaster_core::{run as run_core, Options};
use wasm_bindgen::prelude::*;

fn num(name: &str, raw: &str, default: f64) -> Result<f64, JsValue> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number (got \"{t}\")")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    model: &str,
    horizon: &str,
    season_length: &str,
    alpha: &str,
    beta: &str,
    gamma: &str,
    phi: &str,
    confidence: &str,
    show_fitted: &str,
    header: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let blank = |s: &str, d: &str| {
        if s.trim().is_empty() {
            d.to_string()
        } else {
            s.trim().to_string()
        }
    };
    let opts = Options {
        model: blank(model, "auto"),
        horizon: num("horizon", horizon, 6.0)?.round() as i64,
        season_length: num("season_length", season_length, 0.0)?.round() as i64,
        alpha: num("alpha", alpha, 0.0)?,
        beta: num("beta", beta, 0.0)?,
        gamma: num("gamma", gamma, 0.0)?,
        phi: num("phi", phi, 0.0)?,
        confidence: blank(confidence, "95"),
        // Checkbox sends "true"/"false"; blank (no field) means the default (false).
        show_fitted: matches!(
            show_fitted.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        ),
        header: blank(header, "auto"),
        decimals: num("decimals", decimals, 3.0)?.round() as i64,
        format: blank(format, "text"),
    };
    run_core(data, &opts).map_err(|e| JsValue::from_str(&e))
}
