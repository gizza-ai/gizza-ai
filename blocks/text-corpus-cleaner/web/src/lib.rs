//! Browser-facing wasm-bindgen wrapper for /tools/text-corpus-cleaner/.
//! Field order MUST match meta.toml: input, unicode_form, whitespace, lowercase,
//! dedupe, language, min_chars, min_words, max_symbol_ratio, collapse_blank.
//! Every field arrives as a string (checkboxes send "true"/"false").
use gizza_ai_text_corpus_cleaner_core::run as core_run;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn parse_u32(s: &str, label: &str) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<u32>()
        .map_err(|_| JsValue::from_str(&format!("{label} must be a whole number 0 or greater")))
}

fn parse_ratio(s: &str) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(1.0);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str("max_symbol_ratio must be a number from 0.0 to 1.0"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    unicode_form: &str,
    whitespace: &str,
    lowercase: &str,
    dedupe: &str,
    language: &str,
    min_chars: &str,
    min_words: &str,
    max_symbol_ratio: &str,
    collapse_blank: &str,
) -> Result<String, JsValue> {
    let form = if unicode_form.is_empty() { "nfc" } else { unicode_form };
    let ws = if whitespace.is_empty() { "trim" } else { whitespace };
    let dd = if dedupe.is_empty() { "exact" } else { dedupe };
    let lang = if language.is_empty() { "any" } else { language };
    let mc = parse_u32(min_chars, "min_chars")?;
    let mw = parse_u32(min_words, "min_words")?;
    let ratio = parse_ratio(max_symbol_ratio)?;
    // lowercase defaults OFF: an absent (empty) value means unchecked.
    let lc = truthy(lowercase);
    // collapse_blank defaults ON: an absent (empty) value keeps the default.
    let collapse = collapse_blank.trim().is_empty() || truthy(collapse_blank);
    core_run(input, form, ws, lc, dd, lang, mc, mw, ratio, collapse).map_err(|e| JsValue::from_str(&e))
}
