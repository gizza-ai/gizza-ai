//! Browser-facing wasm-bindgen wrapper for /tools/bech32-codec/.
//! Field order MUST match page/meta.toml: input, mode, hrp, variant, format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    hrp: &str,
    variant: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_bech32_codec_core::convert(input, mode, hrp, variant, format)
        .map_err(|e| JsValue::from_str(&e))
}
