//! Browser-facing wasm-bindgen wrapper for /tools/weighted-random-sampler/.
//! The generic page driver passes every pure-tool field as a string (checkboxes
//! included), so parse numbers/booleans here rather than using JS boolean/number
//! signatures.
use wasm_bindgen::prelude::*;

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    format: &str,
    weight_field: &str,
    n: &str,
    replacement: &str,
    seed: &str,
    header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let n = if n.trim().is_empty() {
        2
    } else {
        n.trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str("n must be a whole number ≥ 1"))?
    };
    let seed = if seed.trim().is_empty() {
        42
    } else {
        seed.trim()
            .parse::<u64>()
            .map_err(|_| JsValue::from_str("seed must be a whole number ≥ 0"))?
    };
    let replacement = matches!(
        replacement.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let header = !matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let format = if format.trim().is_empty() {
        "csv"
    } else {
        format
    };
    let delimiter = if delimiter.trim().is_empty() {
        "comma"
    } else {
        delimiter
    };
    gizza_ai_weighted_random_sampler_core::sample(
        data,
        format,
        weight_field,
        n,
        replacement,
        seed,
        header,
        delimiter,
    )
    .map_err(|e| JsValue::from_str(&e))
}
