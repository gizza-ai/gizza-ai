//! Browser-facing wasm-bindgen wrapper for /tools/cumulative-percent-builder/.
use wasm_bindgen::prelude::*;

fn parse_num(v: &str, fallback: f64, name: &str) -> Result<f64, JsValue> {
    let t = v.trim();
    if t.is_empty() { return Ok(fallback); }
    t.parse::<f64>().map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    header: &str,
    sort: &str,
    threshold: &str,
    top_n: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    let threshold = parse_num(threshold, 80.0, "threshold")?;
    let top_n = parse_num(top_n, 0.0, "top_n")?;
    let decimals = parse_num(decimals, 1.0, "decimals")?;
    if top_n.fract() != 0.0 || decimals.fract() != 0.0 {
        return Err(JsValue::from_str("top_n and decimals must be whole numbers"));
    }
    gizza_ai_cumulative_percent_builder_core::run(
        data,
        if delimiter.trim().is_empty() { "auto" } else { delimiter },
        if header.trim().is_empty() { "auto" } else { header },
        if sort.trim().is_empty() { "desc" } else { sort },
        threshold,
        top_n as usize,
        decimals as usize,
        if output.trim().is_empty() { "table" } else { output },
    )
    .map_err(|e| JsValue::from_str(&e))
}
