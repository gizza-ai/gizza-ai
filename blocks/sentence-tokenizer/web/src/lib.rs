//! Browser-facing wasm-bindgen wrapper for /tools/sentence-tokenizer/.
//! Compiled with wasm-pack for the standalone /tools/sentence-tokenizer/ page.
use wasm_bindgen::prelude::*;

/// Tokenize `text` into sentences and tokens, then render the result.
///
/// The standalone tool page passes every field value as a string, so the
/// non-string params arrive as strings and are parsed here, each falling back
/// to the descriptor's default when the field is left blank:
/// - `format`: `"json"` (default) / `"table"` / `"lines"` / `"spaces"` /
///   `"sentences"`.
/// - `newlines`: `"paragraph"` (default) / `"never"` / `"always"`.
/// - `split_contractions` (default on), `split_hyphenated`, `lowercase` and
///   `drop_punctuation`: `"true"`/`"1"`/`"yes"`/`"on"` vs
///   `"false"`/`"0"`/`"no"`/`"off"`; blank → the descriptor default. The page
///   renders each as a checkbox.
/// - `extra_abbreviations`: free text, blank → none.
///
/// Throws a JS error string on an invalid `format`/`newlines`/checkbox value,
/// on empty or over-long input, or when the filters remove every token.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    format: &str,
    newlines: &str,
    split_contractions: &str,
    split_hyphenated: &str,
    lowercase: &str,
    drop_punctuation: &str,
    extra_abbreviations: &str,
) -> Result<String, JsValue> {
    // Blank selects fall through to the core's own "" → default handling.
    let split_contractions = parse_bool("split_contractions", split_contractions, true)
        .map_err(|e| JsValue::from_str(&e))?;
    let split_hyphenated =
        parse_bool("split_hyphenated", split_hyphenated, false).map_err(|e| JsValue::from_str(&e))?;
    let lowercase = parse_bool("lowercase", lowercase, false).map_err(|e| JsValue::from_str(&e))?;
    let drop_punctuation =
        parse_bool("drop_punctuation", drop_punctuation, false).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_sentence_tokenizer_core::run(
        text,
        format,
        newlines,
        split_contractions,
        split_hyphenated,
        lowercase,
        drop_punctuation,
        extra_abbreviations,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Checkbox / query-param booleans arrive as text; anything unrecognised is a
/// hard error rather than a silent `false`.
fn parse_bool(name: &str, v: &str, default: bool) -> Result<bool, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => Ok(default),
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!(
            "invalid {name} {other:?}: expected \"true\" or \"false\""
        )),
    }
}
