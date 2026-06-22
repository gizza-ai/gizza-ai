//! Browser-facing wasm-bindgen wrapper for /tools/random-token-generator/.
//! Field order MUST match meta.toml: length, count, charset, custom_chars.
use gizza_ai_random_token_generator_core::generate;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(length: f64, count: f64, charset: &str, custom_chars: &str) -> Result<String, JsValue> {
    let len = if length >= 1.0 { length.round() as usize } else { 32 };
    let cnt = if count >= 1.0 { count.round() as usize } else { 1 };
    let cs = if charset.trim().is_empty() { "hex" } else { charset };
    let t = generate(len, cnt, cs, custom_chars).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "{}\n\n({:.1} bits of entropy per token, {}-character alphabet)",
        t.tokens.join("\n"),
        t.bits,
        t.charset_size
    ))
}
