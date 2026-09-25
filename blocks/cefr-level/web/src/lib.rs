//! Browser-facing wasm-bindgen wrapper for /tools/cefr-level/.
//! Field order MUST match meta.toml: text, output, target, coverage, unknown, proper_nouns.
use gizza_ai_cefr_level_core::{run_with_options, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    output: &str,
    target: &str,
    coverage: f64,
    unknown: &str,
    proper_nouns: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        output: if output.trim().is_empty() {
            "summary".into()
        } else {
            output.into()
        },
        target: if target.trim().is_empty() {
            "B1".into()
        } else {
            target.into()
        },
        coverage: if coverage.is_finite() {
            coverage.round() as u32
        } else {
            90
        },
        unknown: if unknown.trim().is_empty() {
            "estimate".into()
        } else {
            unknown.into()
        },
        proper_nouns: truthy(proper_nouns),
    };
    run_with_options(text, &opts).map_err(|e| JsValue::from_str(&e))
}
