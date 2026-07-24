//! Browser-facing wasm-bindgen wrapper for /tools/format-validator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Validate one value as email, URL, IP, phone, card, or auto-detect.
///
/// The page passes args in `page/meta.toml` order: `input`, `format`, `output`.
/// Blank `format` means `auto`; blank `output` means `text`.
#[wasm_bindgen]
pub fn run(input: &str, format: &str, output: &str) -> Result<String, JsValue> {
    gizza_ai_format_validator_core::validate(input, format, output)
        .map_err(|e| JsValue::from_str(&e))
}
