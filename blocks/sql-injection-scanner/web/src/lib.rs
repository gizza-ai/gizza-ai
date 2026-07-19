//! Browser-facing wasm-bindgen wrapper for /tools/sql-injection-scanner/.
//! Field order MUST match meta.toml: code, language, min_severity, format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    code: &str,
    language: &str,
    min_severity: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_sql_injection_scanner_core::run(code, language, min_severity, format)
        .map_err(|e| JsValue::from_str(&e))
}
