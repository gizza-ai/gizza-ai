//! Browser-facing wasm-bindgen wrapper for /tools/url-encode/.
//! Compiled with wasm-pack for the standalone /tools/url-encode/ page.
use wasm_bindgen::prelude::*;

/// Percent-encode or percent-decode `text`. `mode` is `"encode"`/`"decode"`
/// (blank → encode), `target` is `"component"`/`"uri"` (blank → component,
/// ignored on decode). Throws a JS error string on an invalid enum value or an
/// invalid-UTF-8 decode.
#[wasm_bindgen]
pub fn run(text: &str, mode: &str, target: &str) -> Result<String, JsValue> {
    gizza_ai_url_encode_core::convert(text, mode, target).map_err(|e| JsValue::from_str(&e))
}
