//! Browser-facing wasm-bindgen wrapper for /tools/generate-hotp/.
//! Field order MUST match meta.toml: secret, counter, digits, algorithm.
use gizza_ai_generate_hotp_core::{generate, Algo};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(secret: &str, counter: &str, digits: &str, algorithm: &str) -> Result<String, JsValue> {
    let counter: u64 = counter
        .trim()
        .parse()
        .map_err(|_| JsValue::from_str("counter must be a non-negative whole number"))?;
    let digits: u32 = digits.trim().parse().unwrap_or(6);
    let algo = Algo::parse(algorithm).map_err(|e| JsValue::from_str(&e))?;
    let h = generate(secret, counter, digits, algo).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("{}  (counter {})", h.code, h.counter))
}
