//! Browser-facing wasm-bindgen wrapper for /tools/bcrypt-hash/.
//! Field order MUST match meta.toml: password, mode, hash, cost.
use gizza_ai_bcrypt_hash_core::{hash, verify};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(password: &str, mode: &str, hash_str: &str, cost: &str) -> Result<String, JsValue> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "verify" => {
            if hash_str.trim().is_empty() {
                return Err(JsValue::from_str("verify mode requires the bcrypt hash string"));
            }
            let m = verify(password, hash_str).map_err(|e| JsValue::from_str(&e))?;
            Ok(if m { "✓ match".to_string() } else { "✗ no match".to_string() })
        }
        _ => {
            let c: u32 = cost.trim().parse().unwrap_or(12);
            hash(password, c).map_err(|e| JsValue::from_str(&e))
        }
    }
}
