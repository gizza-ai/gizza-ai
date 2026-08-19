//! Browser-facing wasm-bindgen wrapper for /tools/otpauth-uri-parser/.
//! The page passes every field as a string in meta.toml order.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(uri: &str, format: &str, mask_secret: &str, strict: &str) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "json" } else { format };
    gizza_ai_otpauth_uri_parser_core::run_with_options(
        uri,
        format,
        truthy(mask_secret),
        truthy(strict),
    )
    .map_err(|e| JsValue::from_str(&e))
}
