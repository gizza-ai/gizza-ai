//! Browser-facing wasm-bindgen wrapper for /tools/har-request-extract/.
//! Field order MUST match meta.toml: har, format, status, method,
//! url_contains, sort. Fields arrive as strings; empty selects fall back to
//! the descriptor defaults (deep-links may omit any optional param).
use gizza_ai_har_request_extract_core::extract;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    har: &str,
    format: &str,
    status: &str,
    method: &str,
    url_contains: &str,
    sort: &str,
) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "table" } else { format };
    let status = if status.trim().is_empty() { "all" } else { status };
    let sort = if sort.trim().is_empty() { "order" } else { sort };
    extract(har, format, status, method, url_contains, sort).map_err(|e| JsValue::from_str(&e))
}
