//! Browser-facing wasm-bindgen wrapper for /tools/spell-check/.
//! Compiled with wasm-pack for the standalone /tools/spell-check/ page.
use wasm_bindgen::prelude::*;

/// Spell-check `text` and return a human-readable report (misspellings with
/// suggestions + a fully corrected copy).
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here (arg order
/// matches the `page/meta.toml` `[[input]]` order):
/// - `max_suggestions`: `1`–`20` (blank/unparseable → 5; the core clamps 0→none).
/// - `ignore_uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → skip ALL-CAPS acronyms
///   (checkbox default on); anything else → off.
/// - `ignore_capitalized`: same truthy parse → skip Capitalized proper nouns.
/// - `custom_words`: extra correctly-spelled words (commas/spaces/newlines).
#[wasm_bindgen]
pub fn run(
    text: &str,
    max_suggestions: &str,
    ignore_uppercase: &str,
    ignore_capitalized: &str,
    custom_words: &str,
) -> Result<String, JsValue> {
    let max_suggestions = max_suggestions.trim().parse::<usize>().unwrap_or(5);
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    };
    Ok(gizza_ai_spell_check_core::format_report(
        text,
        max_suggestions,
        truthy(ignore_uppercase),
        truthy(ignore_capitalized),
        custom_words,
    ))
}
