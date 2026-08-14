//! Browser-facing wasm-bindgen wrapper for /tools/code-metrics-analyzer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    source: &str,
    language: &str,
    output: &str,
    complexity_threshold: &str,
    max_functions: &str,
    sort: &str,
) -> Result<String, JsValue> {
    let threshold = complexity_threshold.trim().parse::<u32>().unwrap_or(10);
    let max = max_functions.trim().parse::<usize>().unwrap_or(50);
    gizza_ai_code_metrics_analyzer_core::run_with_options(
        source, language, output, threshold, max, sort,
    )
    .map_err(|e| JsValue::from_str(&e))
}
