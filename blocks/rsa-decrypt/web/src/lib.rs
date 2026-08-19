//! Browser-facing wasm-bindgen wrapper for /tools/rsa-decrypt/.
//! Field order MUST match meta.toml: ciphertext, private_key, passphrase,
//! padding, hash, ciphertext_encoding, output_encoding.
use gizza_ai_rsa_decrypt_core::{decrypt, CipherEncoding, Hash, OutputEncoding, Padding};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    ciphertext: &str,
    private_key: &str,
    passphrase: &str,
    padding: &str,
    hash: &str,
    ciphertext_encoding: &str,
    output_encoding: &str,
) -> Result<String, JsValue> {
    let padding = Padding::parse(padding).map_err(|e| JsValue::from_str(&e))?;
    let hash = Hash::parse(hash).map_err(|e| JsValue::from_str(&e))?;
    let cipher_encoding =
        CipherEncoding::parse(ciphertext_encoding).map_err(|e| JsValue::from_str(&e))?;
    let output_encoding =
        OutputEncoding::parse(output_encoding).map_err(|e| JsValue::from_str(&e))?;
    decrypt(
        ciphertext,
        private_key,
        passphrase,
        padding,
        hash,
        cipher_encoding,
        output_encoding,
    )
    .map(|d| d.plaintext)
    .map_err(|e| JsValue::from_str(&e))
}
