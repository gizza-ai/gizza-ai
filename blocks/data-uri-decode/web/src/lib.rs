//! Browser-facing wasm-bindgen wrapper for /tools/data-uri-decode/.
//! Compiled with wasm-pack for the standalone /tools/data-uri-decode/ page.
use wasm_bindgen::prelude::*;

/// Decode a `data:` URI and return a human-readable breakdown (MIME type,
/// charset/params, encoding, byte size, and the decoded text or a hex preview).
/// Throws a JS error string when the input is not a well-formed data URI or its
/// payload can't be decoded.
#[wasm_bindgen]
pub fn run(uri: &str) -> Result<String, JsValue> {
    gizza_ai_data_uri_decode_core::render(uri).map_err(|e| JsValue::from_str(&e))
}
