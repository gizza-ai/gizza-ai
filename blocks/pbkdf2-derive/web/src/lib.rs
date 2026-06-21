//! Browser-facing wasm-bindgen wrapper for /tools/pbkdf2-derive/.
//! Field order MUST match meta.toml: password, mode, salt, salt_encoding,
//! iterations, hash, length, encoding, expected.
use gizza_ai_pbkdf2_derive_core::{derive, verify};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    password: &str,
    mode: &str,
    salt: &str,
    salt_encoding: &str,
    iterations: &str,
    hash: &str,
    length: &str,
    encoding: &str,
    expected: &str,
) -> Result<String, JsValue> {
    let iters: u32 = iterations.trim().parse().unwrap_or(100_000);
    let len: usize = length.trim().parse().unwrap_or(32);
    let salt_enc = if salt_encoding.trim().is_empty() { "utf8" } else { salt_encoding };
    let h = if hash.trim().is_empty() { "sha256" } else { hash };
    let enc = if encoding.trim().is_empty() { "hex" } else { encoding };
    match mode.trim().to_ascii_lowercase().as_str() {
        "verify" => {
            if expected.trim().is_empty() {
                return Err(JsValue::from_str("verify mode requires the expected key"));
            }
            let m = verify(password, salt, salt_enc, iters, h, expected)
                .map_err(|e| JsValue::from_str(&e))?;
            Ok(if m { "✓ match".to_string() } else { "✗ no match".to_string() })
        }
        _ => derive(password, salt, salt_enc, iters, h, len, enc).map_err(|e| JsValue::from_str(&e)),
    }
}
