//! Browser-facing wasm-bindgen wrapper for /tools/disposable-email-detector/.
use wasm_bindgen::prelude::*;

/// Check an email address (or bare domain) against the built-in disposable list
/// and return the human-readable report. Always succeeds (a clean address is
/// reported as `Disposable: no`, not an error).
#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    Ok(gizza_ai_disposable_email_detector_core::report(input))
}
