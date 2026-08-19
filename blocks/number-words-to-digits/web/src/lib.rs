//! Browser-facing wasm-bindgen wrapper for /tools/number-words-to-digits/.
//! Field order MUST match meta.toml.
use gizza_ai_number_words_to_digits_core::convert;
use wasm_bindgen::prelude::*;

/// The page sends every checkbox as "true"/"false"; a missing field arrives empty,
/// in which case the descriptor default applies.
fn flag(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(
            s.to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        ),
    }
}

fn or_default<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    separator: &str,
    scale: &str,
    ordinals: &str,
    fractions: &str,
    digit_sequences: &str,
) -> Result<String, JsValue> {
    convert(
        input,
        or_default(mode, "replace"),
        or_default(separator, "none"),
        or_default(scale, "short"),
        or_default(ordinals, "cardinal"),
        flag(fractions, true),
        flag(digit_sequences, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
