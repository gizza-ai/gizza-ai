//! Browser-facing wasm-bindgen wrapper for /tools/wireguard-keygen/.
//! The argument order mirrors `page/meta.toml`'s `[[input]]` order.
//!
//! `pairs` is `f64` because the page marshals numeric params as JS numbers, and
//! the booleans arrive as the STRINGS "true"/"false" (`readField` on a checkbox)
//! — a wasm-bindgen `bool` param would coerce both to false, so parse truthily.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    pairs: f64,
    preshared_key: &str,
    format: &str,
    address: &str,
    endpoint: &str,
) -> Result<String, JsValue> {
    let n = if pairs.is_finite() && pairs >= 1.0 { pairs } else { 1.0 };
    gizza_ai_wireguard_keygen_core::run(n, truthy(preshared_key), format, address, endpoint)
        .map_err(|e| JsValue::from_str(&e))
}
