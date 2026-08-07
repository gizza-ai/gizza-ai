//! Browser-facing wasm-bindgen wrapper for /tools/json-mask/.
//! Field order MUST match meta.toml: json, mask, mode, format, on_missing, empty.
//! Fields arrive as strings; an empty string selects the option's default.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    json: &str,
    mask: &str,
    mode: &str,
    format: &str,
    on_missing: &str,
    empty: &str,
) -> Result<String, JsValue> {
    gizza_ai_json_mask_core::run(json, mask, mode, format, on_missing, empty)
        .map_err(|e| JsValue::from_str(&e))
}
