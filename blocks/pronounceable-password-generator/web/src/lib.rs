//! Browser-facing wasm-bindgen wrapper for /tools/pronounceable-password-generator/.
//! Field order MUST match meta.toml: length, capitalize, digits, symbols.
use gizza_ai_pronounceable_password_generator_core::generate_pronounceable;
use wasm_bindgen::prelude::*;

fn truthy_default_true(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes" | "")
}

#[wasm_bindgen]
pub fn run(length: f64, capitalize: &str, digits: f64, symbols: f64) -> Result<String, JsValue> {
    let len = if length >= 4.0 { length.round() as usize } else { 12 };
    let dg = if digits >= 0.0 { digits.round() as usize } else { 2 };
    let sy = if symbols >= 0.0 { symbols.round() as usize } else { 1 };
    let (value, bits) = generate_pronounceable(len, truthy_default_true(capitalize), dg, sy)
        .map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("{value}\n\n({bits:.1} bits of entropy)"))
}
