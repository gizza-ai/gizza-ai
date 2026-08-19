//! Browser-facing wasm-bindgen wrapper for /tools/prose-linter/.
use gizza_ai_prose_linter_core::{analyze, OutputFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    text: &str,
    checks: &str,
    output: &str,
    ignore: &str,
    max_issues: f64,
    long_sentence_words: f64,
) -> Result<String, JsValue> {
    let checks = if checks.trim().is_empty() {
        "all"
    } else {
        checks
    };
    let output = if output.trim().is_empty() {
        "report"
    } else {
        output
    };
    let fmt = OutputFormat::parse(output).map_err(|e| JsValue::from_str(&e))?;
    let max_issues = if max_issues.is_finite() && max_issues >= 0.0 {
        max_issues as usize
    } else {
        200
    };
    let long_sentence_words = if long_sentence_words.is_finite() && long_sentence_words >= 0.0 {
        long_sentence_words as usize
    } else {
        30
    };
    analyze(
        text,
        checks,
        fmt,
        ignore,
        max_issues,
        long_sentence_words,
    )
    .map_err(|e| JsValue::from_str(&e))
}
