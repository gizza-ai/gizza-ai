//! Browser-facing wasm-bindgen wrapper for /tools/xpub-derive/.
//! The page hands every field through as a string, so parsing lives in the core.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    xpub: &str,
    chain: &str,
    count: &str,
    start: &str,
    address_type: &str,
    format: &str,
    include_public_key: &str,
) -> Result<String, JsValue> {
    gizza_ai_xpub_derive_core::derive_str(
        xpub,
        chain,
        count,
        start,
        address_type,
        format,
        include_public_key,
    )
    .map_err(|e| JsValue::from_str(&e))
}
