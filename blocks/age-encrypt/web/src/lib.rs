//! Browser-facing wasm-bindgen wrapper for /tools/age-encrypt/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    passphrase: &str,
    recipients: &str,
    work_factor: &str,
) -> Result<String, JsValue> {
    let wf = if work_factor.trim().is_empty() {
        gizza_ai_age_encrypt_core::DEFAULT_WORK_FACTOR
    } else {
        work_factor.trim().parse::<u8>().map_err(|_| {
            JsValue::from_str("work_factor must be a whole number between 10 and 15")
        })?
    };
    gizza_ai_age_encrypt_core::run(text, mode, passphrase, recipients, wf)
        .map_err(|e| JsValue::from_str(&e))
}
