//! Browser-facing wasm-bindgen wrapper for /tools/csv-sample/.
//! The page passes every field as a raw string (no coercion for pure tools),
//! so numeric params arrive as &str and are parsed here (blank → default).
use gizza_ai_csv_sample_core::sample;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    method: &str,
    n: &str,
    percent: &str,
    stratify_column: &str,
    seed: &str,
    header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let n = if n.trim().is_empty() {
        10
    } else {
        n.trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str("n must be a whole number ≥ 1"))?
    };
    let percent = if percent.trim().is_empty() {
        0.0
    } else {
        percent
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str("percent must be a number between 0 and 100"))?
    };
    let seed = if seed.trim().is_empty() {
        42
    } else {
        seed.trim()
            .parse::<u64>()
            .map_err(|_| JsValue::from_str("seed must be a whole number ≥ 0"))?
    };
    let hdr = !matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let method = if method.trim().is_empty() { "random" } else { method };
    let delim = if delimiter.trim().is_empty() { "comma" } else { delimiter };
    sample(data, method, n, percent, stratify_column, seed, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
