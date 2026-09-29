//! Browser-facing wasm-bindgen wrapper for /tools/caesar-cipher-breaker/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    output: &str,
    language: &str,
    top: &str,
    shift_digits: &str,
) -> Result<String, JsValue> {
    let top = if top.trim().is_empty() {
        5
    } else {
        top.trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("top must be a number between 1 and 26"))?
    };
    gizza_ai_caesar_cipher_breaker_core::crack(
        input,
        or_default(output, "best"),
        or_default(language, "english"),
        top,
        truthy(shift_digits),
    )
    .map_err(|e| JsValue::from_str(&e))
}
