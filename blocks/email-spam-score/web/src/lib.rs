//! Browser-facing wasm-bindgen wrapper for /tools/email-spam-score/.
use wasm_bindgen::prelude::*;

/// Score a pasted email and return the transparent, rule-by-rule report.
///
/// `check_headers` arrives from the page driver as `"true"`/`"false"` (see the
/// create-next-tool page-patterns note on boolean checkbox marshaling), so parse it
/// positive-truthy rather than trusting a bare non-empty string.
#[wasm_bindgen]
pub fn run(
    email: &str,
    subject: &str,
    format: &str,
    report: &str,
    check_headers: &str,
) -> Result<String, JsValue> {
    let check_headers = matches!(
        check_headers.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_email_spam_score_core::run(email, subject, format, report, check_headers)
        .map_err(|e| JsValue::from_str(&e))
}
