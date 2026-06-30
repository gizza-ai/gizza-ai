//! Browser-facing wasm-bindgen wrapper for /tools/sm2-public-from-private/.
//! Field order MUST match meta.toml: private_key, input_format, output_format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(private_key: &str, input_format: &str, output_format: &str) -> Result<String, JsValue> {
    gizza_ai_sm2_public_from_private_core::derive_formatted(
        private_key,
        input_format,
        output_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
