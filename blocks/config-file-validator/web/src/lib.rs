//! Browser-facing wasm-bindgen wrapper for /tools/config-file-validator/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    format: &str,
    strict: &str,
    report_format: &str,
    context_lines: &str,
) -> Result<String, JsValue> {
    let context_lines = context_lines
        .trim()
        .parse::<usize>()
        .map_err(|_| JsValue::from_str("context_lines must be a whole number from 0 to 10"))?;
    gizza_ai_config_file_validator_core::validate(
        input,
        format,
        truthy(strict),
        report_format,
        context_lines,
    )
    .map_err(|e| JsValue::from_str(&e))
}
