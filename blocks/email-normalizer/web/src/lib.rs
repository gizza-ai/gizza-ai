//! Browser-facing wasm-bindgen wrapper for /tools/email-normalizer/.
//! Compiled with wasm-pack for the standalone /tools/email-normalizer/ page.
use wasm_bindgen::prelude::*;

/// Canonicalize `email` and return the multi-line report.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean `lowercase_local` arrives as a string and is parsed here — the box
/// is checked by default (`"true"`), unchecked sends `"false"`.
///
/// Throws a JS error string on a syntactically invalid address.
#[wasm_bindgen]
pub fn run(email: &str, lowercase_local: &str) -> Result<String, JsValue> {
    let lowercase_local = matches!(
        lowercase_local.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_email_normalizer_core::report(email, lowercase_local)
        .map_err(|e| JsValue::from_str(&e))
}
