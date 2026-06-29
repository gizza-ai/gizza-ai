//! Browser-facing wasm-bindgen wrapper for /tools/slugify/.
//! Compiled with wasm-pack for the standalone /tools/slugify/ page.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"yes"`/`"on"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Slugify `text`.
///
/// Field order MUST match meta.toml: text, separator, lowercase, max_length,
/// per_line. The standalone tool page passes every field value as a string, so
/// the boolean/integer params arrive as strings and are parsed here:
/// - `separator`: the word separator (blank → `-`).
/// - `lowercase`: truthy → lowercase (this is the default-checked state);
///   anything else → keep original case.
/// - `max_length`: a count (blank/unparseable → `0` = no limit; core clamps it).
/// - `per_line`: truthy → slugify each line of `text` independently (a batch
///   list of titles), rejoined with newlines; otherwise the whole input is one
///   slug.
///
/// Throws a JS error string when `separator` contains ASCII letters/digits.
#[wasm_bindgen]
pub fn run(
    text: &str,
    separator: &str,
    lowercase: &str,
    max_length: &str,
    per_line: &str,
) -> Result<String, JsValue> {
    let max_length = max_length.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_slugify_core::slugify(
        text,
        separator,
        truthy(lowercase),
        max_length,
        truthy(per_line),
    )
    .map_err(|e| JsValue::from_str(&e))
}
