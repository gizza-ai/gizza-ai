//! Browser-facing wasm-bindgen wrapper for /tools/smart-quotes-clean/.
//! Compiled with wasm-pack for the standalone /tools/smart-quotes-clean/ page.
use gizza_ai_smart_quotes_clean_core::{clean, EmDash};
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"yes"`/`"on"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Clean `text` of smart quotes and other typographic characters.
///
/// Field order MUST match meta.toml: text, em_dash, normalize_spaces. The page
/// passes every field value as a string, so `normalize_spaces` arrives as a
/// string and is parsed here; `em_dash` is one of the select values
/// (`--` / `-` / ` - `) and is mapped by the core (blank → default `--`).
#[wasm_bindgen]
pub fn run(text: &str, em_dash: &str, normalize_spaces: &str) -> Result<String, JsValue> {
    Ok(clean(
        text,
        EmDash::parse(em_dash),
        truthy(normalize_spaces),
    ))
}
