//! Browser-facing wasm-bindgen wrapper for /tools/pkcs12-inspect/.
//! Compiled with wasm-pack for the standalone page: the pasted base64/hex
//! container plus the encoding and output-format selects. Everything is parsed
//! locally — the file never leaves the browser and no password is needed.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, encoding: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_pkcs12_inspect_core::run(data, encoding, format).map_err(|e| JsValue::from_str(&e))
}
