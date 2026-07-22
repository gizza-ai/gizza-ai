//! Browser-facing wasm-bindgen wrapper for /tools/fuzzy-dedupe/.
//! Field values arrive as strings from the page; booleans as "true"/"false".
use gizza_ai_fuzzy_dedupe_core::dedupe;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    delimiter: &str,
    header: &str,
    threshold: &str,
    keep: &str,
    normalize_case: &str,
    normalize_spacing: &str,
    output: &str,
) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let thr = threshold.trim().parse::<f64>().unwrap_or(85.0);
    let keep = if keep.is_empty() { "first" } else { keep };
    let output = if output.is_empty() { "deduped" } else { output };
    dedupe(
        data,
        columns,
        delim,
        truthy(header),
        thr,
        keep,
        truthy(normalize_case),
        truthy(normalize_spacing),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
