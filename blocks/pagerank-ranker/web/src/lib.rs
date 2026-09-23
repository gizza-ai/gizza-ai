//! Browser-facing wasm-bindgen wrapper for /tools/pagerank-ranker/.
use gizza_ai_pagerank_ranker_core::Options;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    directed: &str,
    weighted: &str,
    damping: &str,
    max_iter: &str,
    tolerance: &str,
    dangling: &str,
    personalization: &str,
    top: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let defaults = Options::default();
    let opts = Options {
        input_format: if input_format.trim().is_empty() {
            defaults.input_format
        } else {
            input_format.into()
        },
        directed: truthy(directed),
        weighted: truthy(weighted),
        damping: damping.trim().parse().unwrap_or(defaults.damping),
        max_iter: max_iter.trim().parse().unwrap_or(defaults.max_iter),
        tolerance: tolerance.trim().parse().unwrap_or(defaults.tolerance),
        dangling: if dangling.trim().is_empty() {
            defaults.dangling
        } else {
            dangling.into()
        },
        personalization: personalization.into(),
        top: top.trim().parse().unwrap_or(defaults.top),
        decimals: decimals.trim().parse().unwrap_or(defaults.decimals),
        format: if format.trim().is_empty() {
            defaults.format
        } else {
            format.into()
        },
    };
    gizza_ai_pagerank_ranker_core::run(input, &opts).map_err(|e| JsValue::from_str(&e))
}
