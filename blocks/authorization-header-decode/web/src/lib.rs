//! Browser-facing wasm-bindgen wrapper for /tools/authorization-header-decode/.
//! The page passes every field as a string in meta.toml order.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    header: &str,
    format: &str,
    mask_credentials: &str,
    strict: &str,
) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "json" } else { format };
    gizza_ai_authorization_header_decode_core::run_with_options(
        header,
        format,
        truthy(mask_credentials),
        truthy(strict),
    )
    .map_err(|e| JsValue::from_str(&e))
}
