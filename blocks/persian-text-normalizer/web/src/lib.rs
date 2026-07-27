//! Browser-facing wasm-bindgen wrapper for /tools/persian-text-normalizer/.
//! Field order MUST match meta.toml: text, characters, digits, half_space,
//! punctuation_spacing, persian_punctuation, remove_diacritics, whitespace.
//! Fields arrive as strings (checkboxes send "true"/"false"; an empty select
//! falls back to the core parse default).
use gizza_ai_persian_text_normalizer_core::{normalize, Digits, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    characters: &str,
    digits: &str,
    half_space: &str,
    punctuation_spacing: &str,
    persian_punctuation: &str,
    remove_diacritics: &str,
    whitespace: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        characters: truthy(characters),
        digits: Digits::parse(digits).map_err(|e| JsValue::from_str(&e))?,
        half_space: truthy(half_space),
        punctuation_spacing: truthy(punctuation_spacing),
        persian_punctuation: truthy(persian_punctuation),
        remove_diacritics: truthy(remove_diacritics),
        whitespace: truthy(whitespace),
    };
    Ok(normalize(text, opts))
}
