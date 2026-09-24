//! Browser-facing wasm-bindgen wrapper for /tools/combinations-permutations/.
//! Field order MUST match meta.toml: items, n, r, mode, repetition,
//! output_format, item_separator, dedupe, join_separator,
//! custom_join_separator, max_results.
use gizza_ai_combinations_permutations_core::{
    compute, parse_item_separator, parse_join_separator, parse_mode, parse_output_format,
};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// An empty number field arrives as NaN — fall back to the supplied default.
fn whole(v: f64, fallback: u64) -> u64 {
    if v.is_finite() && v >= 0.0 {
        v as u64
    } else {
        fallback
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    items: &str,
    n: f64,
    r: f64,
    mode: &str,
    repetition: &str,
    output_format: &str,
    item_separator: &str,
    dedupe: &str,
    join_separator: &str,
    custom_join_separator: &str,
    max_results: f64,
) -> Result<String, JsValue> {
    let mode = parse_mode(mode).map_err(|e| JsValue::from_str(&e))?;
    let out_format = parse_output_format(output_format).map_err(|e| JsValue::from_str(&e))?;
    let item_sep = parse_item_separator(item_separator).map_err(|e| JsValue::from_str(&e))?;
    let join_sep = parse_join_separator(join_separator).map_err(|e| JsValue::from_str(&e))?;
    compute(
        items,
        whole(n, 0),
        whole(r, 0),
        mode,
        truthy(repetition),
        item_sep,
        truthy(dedupe),
        out_format,
        join_sep,
        custom_join_separator,
        whole(max_results, 10_000).max(1),
    )
    .map_err(|e| JsValue::from_str(&e))
}
