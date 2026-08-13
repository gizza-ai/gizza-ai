//! Browser-facing wasm-bindgen wrapper for /tools/age-keygen/.
//! The argument order mirrors `page/meta.toml`'s `[[input]]` order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    format: &str,
    comment: &str,
    include_created: bool,
    seed_or_identity: &str,
) -> Result<String, JsValue> {
    gizza_ai_age_keygen_core::run(format, comment, include_created, seed_or_identity)
        .map_err(|e| JsValue::from_str(&e))
}
