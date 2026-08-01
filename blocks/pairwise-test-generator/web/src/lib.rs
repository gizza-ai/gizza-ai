//! Browser-facing wasm-bindgen wrapper for /tools/pairwise-test-generator/.
//! Field order MUST match meta.toml: parameters, output_format, include_index.
use gizza_ai_pairwise_test_generator_core::{generate, OutputFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(parameters: &str, output_format: &str, include_index: &str) -> Result<String, JsValue> {
    let fmt = OutputFormat::parse(output_format).map_err(|e| JsValue::from_str(&e))?;
    let idx = matches!(
        include_index.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    generate(parameters, fmt, idx).map_err(|e| JsValue::from_str(&e))
}
