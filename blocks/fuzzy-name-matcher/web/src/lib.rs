//! Browser-facing wasm-bindgen wrapper for /tools/fuzzy-name-matcher/.
//! Field values arrive as strings from the page; booleans as "true"/"false".
use gizza_ai_fuzzy_name_matcher_core::match_names;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    names: &str,
    algorithm: &str,
    mode: &str,
    threshold: &str,
    normalize_case: &str,
    ignore_titles: &str,
    output: &str,
) -> Result<String, JsValue> {
    let algorithm = if algorithm.is_empty() { "jaro_winkler" } else { algorithm };
    let mode = if mode.is_empty() { "groups" } else { mode };
    let thr = threshold.trim().parse::<f64>().unwrap_or(85.0);
    match_names(
        names,
        algorithm,
        mode,
        thr,
        truthy(normalize_case),
        truthy(ignore_titles),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
