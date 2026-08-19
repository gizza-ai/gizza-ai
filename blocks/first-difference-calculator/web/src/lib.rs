//! Browser-facing wasm-bindgen wrapper for /tools/first-difference-calculator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Parse an optional integer field. The tool page hands every field value over
/// as a string, so a blank field means "use the default" rather than an error.
fn int_field<T: std::str::FromStr>(raw: &str, label: &str, default: T) -> Result<T, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<T>()
        .map_err(|_| format!("{label} must be a whole number (got {t:?})"))
}

/// Compute the differences of `series`. Returns pretty-printed JSON
/// `{count, lag, order, mode, decimals, drop_warmup, values[], indices[],
/// summary{…}, interpretation}`; throws a JS error string on invalid input.
#[wasm_bindgen]
pub fn run(
    series: &str,
    lag: &str,
    order: &str,
    mode: &str,
    decimals: &str,
    drop_warmup: &str,
) -> Result<String, JsValue> {
    let lag = int_field::<i64>(lag, "lag", 1).map_err(|e| JsValue::from_str(&e))?;
    let order = int_field::<u32>(order, "order", 1).map_err(|e| JsValue::from_str(&e))?;
    let decimals = int_field::<u32>(decimals, "decimals", 6).map_err(|e| JsValue::from_str(&e))?;
    let drop_warmup = matches!(
        drop_warmup.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let d = gizza_ai_first_difference_calculator_core::compute(
        series,
        lag,
        order,
        mode,
        decimals,
        drop_warmup,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string_pretty(&d).map_err(|e| JsValue::from_str(&e.to_string()))
}
