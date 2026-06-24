//! Browser-facing wasm-bindgen wrapper for /tools/email-obfuscator/.
//! Compiled with wasm-pack for the standalone /tools/email-obfuscator/ page.
use wasm_bindgen::prelude::*;

/// Obfuscate `email` into a paste-ready HTML snippet.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean `link` arrives as `"true"`/`"false"` (a default-checked checkbox
/// sends `"true"`):
/// - `mode`: `"entities"` (blank) / `"js"` / `"css"` / `"rot13"`.
/// - `entity_style`: `"decimal"` (blank) / `"hex"`.
/// - `link`: truthy (`"true"`/`"1"`/`"on"`/`"yes"`) → wrap in a mailto: anchor.
/// - `link_text`: optional visible link text.
///
/// Throws a JS error string on an invalid email or an unknown mode/style.
#[wasm_bindgen]
pub fn run(
    email: &str,
    mode: &str,
    entity_style: &str,
    link: &str,
    link_text: &str,
) -> Result<String, JsValue> {
    let link = matches!(
        link.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_email_obfuscator_core::obfuscate(
        email,
        &gizza_ai_email_obfuscator_core::Options {
            mode,
            entity_style,
            link,
            link_text,
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
