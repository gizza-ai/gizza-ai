//! Browser-facing wasm-bindgen wrapper for /tools/email-reply-cleaner/.
//! Compiled with wasm-pack for the standalone /tools/email-reply-cleaner/ page.
use wasm_bindgen::prelude::*;

/// Positive-truthy parse of a page checkbox value (`"true"`/`"1"`/`"yes"`/`"on"`).
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Clean a replied/forwarded email. The standalone tool page passes every field
/// value as a string; the four boolean checkboxes arrive as `"true"`/`"false"`
/// and are parsed here.
///
/// Throws a JS error string when `text` is empty/whitespace.
#[wasm_bindgen]
pub fn run(
    text: &str,
    remove_quotes: &str,
    remove_reply_chain: &str,
    remove_signature: &str,
    collapse_blank_lines: &str,
) -> Result<String, JsValue> {
    gizza_ai_email_reply_cleaner_core::run(
        text,
        truthy(remove_quotes),
        truthy(remove_reply_chain),
        truthy(remove_signature),
        truthy(collapse_blank_lines),
    )
    .map_err(|e| JsValue::from_str(&e))
}
