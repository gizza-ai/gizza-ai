//! Browser-facing wasm-bindgen wrapper for /tools/text-encrypt/.
//! Field order MUST match meta.toml: text, passphrase, mode.
use gizza_ai_text_encrypt_core::{decrypt_text, encrypt_text};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, passphrase: &str, mode: &str) -> Result<String, JsValue> {
    let r = match mode.trim().to_ascii_lowercase().as_str() {
        "encrypt" | "" => encrypt_text(text, passphrase),
        "decrypt" => decrypt_text(text, passphrase),
        other => Err(format!("unknown mode '{other}' (use 'encrypt' or 'decrypt')")),
    };
    r.map_err(|e| JsValue::from_str(&e))
}
