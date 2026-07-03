//! Browser-facing wasm-bindgen wrapper for /tools/cartesian-product/.
//! Field order MUST match meta.toml: list1, list2, list3, list4,
//! item_separator, dedupe, output_format, join_separator,
//! custom_join_separator, prefix, suffix, max_combinations.
use gizza_ai_cartesian_product_core::{
    generate, parse_item_separator, parse_join_separator, parse_output_format,
};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    list1: &str,
    list2: &str,
    list3: &str,
    list4: &str,
    item_separator: &str,
    dedupe: &str,
    output_format: &str,
    join_separator: &str,
    custom_join_separator: &str,
    prefix: &str,
    suffix: &str,
    max_combinations: f64,
) -> Result<String, JsValue> {
    let item_sep = parse_item_separator(item_separator).map_err(|e| JsValue::from_str(&e))?;
    let out_format = parse_output_format(output_format).map_err(|e| JsValue::from_str(&e))?;
    let join_sep = parse_join_separator(join_separator).map_err(|e| JsValue::from_str(&e))?;
    // An empty number field arrives as NaN — fall back to the default cap.
    let cap = if max_combinations.is_finite() && max_combinations >= 1.0 {
        max_combinations as u64
    } else {
        10_000
    };
    generate(
        &[list1, list2, list3, list4],
        item_sep,
        truthy(dedupe),
        out_format,
        join_sep,
        custom_join_separator,
        prefix,
        suffix,
        cap,
    )
    .map_err(|e| JsValue::from_str(&e))
}
