//! Browser-facing wasm-bindgen wrapper for /tools/docker-compose-validator/.
//! The page hands every field over as a string (checkboxes arrive as
//! "true"/"false"), so this delegates to the core's string entry point.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    preset: &str,
    disable: &str,
    strict_warnings: &str,
    min_severity: &str,
    report_format: &str,
) -> Result<String, JsValue> {
    gizza_ai_docker_compose_validator_core::run_str(
        input,
        preset,
        disable,
        strict_warnings,
        min_severity,
        report_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
