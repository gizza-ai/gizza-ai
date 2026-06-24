//! Browser-facing wasm-bindgen wrapper for /tools/aes-key-wrap/.
//! Field order MUST match meta.toml: data, operation, key, algorithm, format.
use gizza_ai_aes_key_wrap_core::{unwrap, wrap, Algorithm, Encoding};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    key: &str,
    algorithm: &str,
    format: &str,
) -> Result<String, JsValue> {
    let alg = Algorithm::parse(algorithm).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let r = match operation.trim().to_ascii_lowercase().as_str() {
        "unwrap" => unwrap(data, key, alg, fmt),
        _ => wrap(data, key, alg, fmt),
    };
    r.map_err(|e| JsValue::from_str(&e))
}
