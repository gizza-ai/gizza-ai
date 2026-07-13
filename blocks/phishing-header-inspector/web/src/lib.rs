//! Browser-facing wasm-bindgen wrapper for /tools/phishing-header-inspector/.
use wasm_bindgen::prelude::*;

/// Inspect pasted email headers and return a human-readable risk report.
#[wasm_bindgen]
pub fn run(headers: &str, report_mode: &str, check_received: bool) -> Result<String, JsValue> {
    gizza_ai_phishing_header_inspector_core::run(headers, report_mode, check_received)
        .map_err(|e| JsValue::from_str(&e))
}
