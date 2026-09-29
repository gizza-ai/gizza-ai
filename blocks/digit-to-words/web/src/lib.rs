//! Browser-facing wasm-bindgen wrapper for /tools/digit-to-words/.
//! The page marshals every field as a string, so checkbox strings are parsed
//! here and blank selects fall back to the descriptor defaults before the pure
//! core receives typed arguments.
use wasm_bindgen::prelude::*;

fn truthy_default_on(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

fn truthy_default_off(v: &str) -> bool {
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
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    style: &str,
    scale: &str,
    letter_case: &str,
    currency: &str,
    use_and: &str,
    hyphenate: &str,
    decimals: &str,
    only_suffix: &str,
) -> Result<String, JsValue> {
    gizza_ai_digit_to_words_core::convert(
        input,
        or_default(style, "cardinal"),
        or_default(scale, "short"),
        or_default(letter_case, "lower"),
        or_default(currency, "USD"),
        truthy_default_off(use_and),
        truthy_default_on(hyphenate),
        or_default(decimals, "point"),
        truthy_default_off(only_suffix),
    )
    .map_err(|e| JsValue::from_str(&e))
}
