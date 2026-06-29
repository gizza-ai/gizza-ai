//! Browser-facing wasm-bindgen wrapper for /tools/page-weight-analyzer/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Analyze pasted HTML and return a page-weight report.
///
/// The standalone tool page passes every field value as a string:
/// - `html`: the full HTML source to analyze.
/// - `output`: `"report"` (default) or `"json"` (blank → report).
/// - `list_resources`: `"true"`/`"1"`/`"yes"`/`"on"` → include the per-resource
///   URL listing; the checkbox defaults to unchecked (false).
///
/// Throws a JS error string on empty HTML or an invalid output format.
#[wasm_bindgen]
pub fn run(html: &str, output: &str, list_resources: &str) -> Result<String, JsValue> {
    gizza_ai_page_weight_analyzer_core::analyze(html, output, truthy(list_resources))
        .map_err(|e| JsValue::from_str(&e))
}

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
