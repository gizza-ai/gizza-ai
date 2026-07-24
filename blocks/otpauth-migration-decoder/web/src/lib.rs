//! Browser-facing wasm-bindgen wrapper for /tools/otpauth-migration-decoder/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(payload: &str, format: &str) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "uri" } else { format };
    gizza_ai_otpauth_migration_decoder_core::run_with_format(payload, format)
        .map_err(|e| JsValue::from_str(&e))
}
