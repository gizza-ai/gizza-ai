//! Browser-facing wasm-bindgen wrapper for /tools/totp-generator/.
//! Field order MUST match meta.toml: secret, digits, algorithm.
//! The current time comes from the browser (`Date.now()`), since
//! wasm32-unknown-unknown has no std clock.
use gizza_ai_totp_generator_core::{generate, Algo};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(secret: &str, digits: &str, algorithm: &str) -> Result<String, JsValue> {
    let digits: u32 = digits.trim().parse().unwrap_or(6);
    let algo = Algo::parse(algorithm).map_err(|e| JsValue::from_str(&e))?;
    let now = (js_sys::Date::now() / 1000.0) as u64;
    let t = generate(secret, now, digits, 30, algo).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("{}  (valid for {}s)", t.code, t.seconds_remaining))
}
