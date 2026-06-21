//! Browser-facing wasm-bindgen wrapper for /tools/argon2-hash/.
//! Field order MUST match meta.toml: password, mode, hash, memory_kib,
//! iterations, parallelism.
use gizza_ai_argon2_hash_core::{hash, verify};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    password: &str,
    mode: &str,
    hash_str: &str,
    memory_kib: &str,
    iterations: &str,
    parallelism: &str,
) -> Result<String, JsValue> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "verify" => {
            if hash_str.trim().is_empty() {
                return Err(JsValue::from_str("verify mode requires the hash PHC string"));
            }
            let m = verify(password, hash_str).map_err(|e| JsValue::from_str(&e))?;
            Ok(if m { "✓ match".to_string() } else { "✗ no match".to_string() })
        }
        _ => {
            let m: u32 = memory_kib.trim().parse().unwrap_or(19456);
            let t: u32 = iterations.trim().parse().unwrap_or(2);
            let p: u32 = parallelism.trim().parse().unwrap_or(1);
            hash(password, m, t, p).map_err(|e| JsValue::from_str(&e))
        }
    }
}
