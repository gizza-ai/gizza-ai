//! Browser-facing wasm-bindgen wrapper for /tools/password-generator/.
//! Field order MUST match meta.toml: mode, length, words, uppercase, digits, symbols, separator.
use gizza_ai_password_generator_core::{generate_passphrase, generate_password};
use wasm_bindgen::prelude::*;

fn truthy_default_true(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
pub fn run(mode: &str, length: f64, words: f64, uppercase: &str, digits: &str, symbols: &str, separator: &str) -> Result<String, JsValue> {
    let res = if mode.trim().eq_ignore_ascii_case("passphrase") {
        let w = if words >= 1.0 { words.round() as usize } else { 4 };
        let sep = if separator.is_empty() { "-" } else { separator };
        generate_passphrase(w, sep)
    } else {
        let len = if length >= 1.0 { length.round() as usize } else { 16 };
        generate_password(len, truthy_default_true(uppercase), truthy_default_true(digits), truthy_default_true(symbols))
    };
    let (value, bits) = res.map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("{value}\n\n({bits:.1} bits of entropy)"))
}
