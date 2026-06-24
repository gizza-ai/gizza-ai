//! Browser-facing wasm-bindgen wrapper for /tools/ja4-server-fingerprint/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(server_hello: &str, quic: &str) -> Result<String, JsValue> {
    let is_quic = matches!(quic, "true" | "1" | "on" | "yes");
    gizza_ai_ja4_server_fingerprint_core::render(server_hello, is_quic)
        .map_err(|e| JsValue::from_str(&e))
}
