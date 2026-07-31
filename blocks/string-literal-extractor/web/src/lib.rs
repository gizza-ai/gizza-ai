//! Browser-facing wasm-bindgen wrapper for /tools/string-literal-extractor/.
//! Arg order matches the page's meta.toml `[[input]]` declaration order; the
//! page passes every field as a string, so booleans/integers are coerced here
//! before delegating to the shared core tokenizer.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    code: &str,
    language: &str,
    quotes: &str,
    format: &str,
    decode_escapes: &str,
    unique: &str,
    min_length: &str,
    line_numbers: &str,
) -> Result<String, JsValue> {
    let min_length = min_length.trim().parse::<i64>().unwrap_or(0);
    gizza_ai_string_literal_extractor_core::extract(
        code,
        language,
        quotes,
        truthy(decode_escapes),
        truthy(unique),
        min_length,
        format,
        truthy(line_numbers),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
