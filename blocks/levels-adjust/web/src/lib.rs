//! Browser-facing wasm-bindgen wrapper for /tools/levels-adjust/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_num(name: &str, s: &str, default: f64) -> Result<f64, JsValue> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    let n: f64 = trimmed
        .parse()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))?;
    if !n.is_finite() {
        return Err(JsValue::from_str(&format!("{name} must be finite")));
    }
    Ok(n)
}

#[wasm_bindgen]
pub fn run(
    values: &str,
    input_black: &str,
    input_white: &str,
    gamma: &str,
    output_black: &str,
    output_white: &str,
    clamp: &str,
) -> Result<String, JsValue> {
    gizza_ai_levels_adjust_core::summary(
        values,
        parse_num("input_black", input_black, 0.0)?,
        parse_num("input_white", input_white, 255.0)?,
        parse_num("gamma", gamma, 1.0)?,
        parse_num("output_black", output_black, 0.0)?,
        parse_num("output_white", output_white, 255.0)?,
        truthy(clamp),
    )
    .map_err(|e| JsValue::from_str(&e))
}
