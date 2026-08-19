//! Browser-facing wasm-bindgen wrapper for /tools/multilingual-stemmer/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    language: &str,
    output: &str,
    min_length: &str,
    lowercase: &str,
) -> Result<String, JsValue> {
    let min_length = if min_length.trim().is_empty() {
        1
    } else {
        min_length
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("min_length must be a whole number between 1 and 30"))?
    };
    gizza_ai_multilingual_stemmer_core::run(input, language, output, min_length, truthy(lowercase))
        .map_err(|e| JsValue::from_str(&e))
}
