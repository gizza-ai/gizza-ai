//! Browser-facing wasm-bindgen wrapper for /tools/category-canonicalize/.
//! Field values arrive as strings from the page; booleans as "true"/"false".
use gizza_ai_category_canonicalize_core::canonicalize;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    mapping: &str,
    column: &str,
    delimiter: &str,
    header: &str,
    ignore_case: &str,
    ignore_spacing: &str,
    unmatched: &str,
    fuzzy_threshold: &str,
    output: &str,
) -> Result<String, JsValue> {
    let delim = if delimiter.trim().is_empty() { "auto" } else { delimiter };
    let thr = fuzzy_threshold.trim().parse::<f64>().unwrap_or(85.0);
    canonicalize(
        data,
        mapping,
        column,
        delim,
        truthy(header),
        truthy(ignore_case),
        truthy(ignore_spacing),
        unmatched,
        thr,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
