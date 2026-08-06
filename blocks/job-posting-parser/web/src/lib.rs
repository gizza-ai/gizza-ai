//! Browser-facing wasm-bindgen wrapper for /tools/job-posting-parser/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(posting: &str, output: &str, include_evidence: &str) -> Result<String, JsValue> {
    fn truthy(s: &str, default: bool) -> bool {
        match s.trim().to_ascii_lowercase().as_str() {
            "" => default,
            "true" | "1" | "on" | "yes" => true,
            "false" | "0" | "off" | "no" => false,
            _ => default,
        }
    }

    gizza_ai_job_posting_parser_core::parse_job_posting(
        posting,
        output,
        truthy(include_evidence, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
