//! Browser-facing wasm-bindgen wrapper for /tools/email-validator/.
use wasm_bindgen::prelude::*;

/// Validate an email address and return the human-readable report. Always
/// succeeds (an invalid address is reported as `Valid: no`, not an error).
#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    Ok(gizza_ai_email_validator_core::report(input))
}
