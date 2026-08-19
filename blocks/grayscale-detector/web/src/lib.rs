//! Browser-facing wasm-bindgen wrapper for /tools/grayscale-detector/.
use wasm_bindgen::prelude::*;

fn parse_whole_field(s: &str, field: &str, hi: u32, fallback: u32) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    let n: u32 = t
        .parse()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number 0-{hi} (got {t:?})")))?;
    if n > hi {
        return Err(JsValue::from_str(&format!(
            "{field} must be 0-{hi} (got {n})"
        )));
    }
    Ok(n)
}

fn parse_bool_field(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    tolerance: &str,
    metric: &str,
    ignore_alpha: &str,
    max_samples: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_grayscale_detector_core::run(
        input,
        input_format,
        parse_whole_field(tolerance, "tolerance", 255, 2)? as u8,
        metric,
        parse_bool_field(ignore_alpha),
        parse_whole_field(max_samples, "max_samples", 200, 20)?,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
