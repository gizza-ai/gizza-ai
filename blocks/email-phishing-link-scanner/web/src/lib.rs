//! Browser-facing wasm-bindgen wrapper for /tools/email-phishing-link-scanner/.
//! tool.js passes every page field as a raw string; param name/order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

/// Scan a pasted email and return the per-link phishing report.
///
/// `only_flagged` arrives from the page driver as `"true"`/`"false"` (see the create-next-tool
/// page-patterns note on boolean checkbox marshaling), so parse it positive-truthy rather than
/// trusting a bare non-empty string. `max_links` arrives as a string too; an empty or unparsable
/// value falls back to the descriptor default so the page never errors on a blank field.
#[wasm_bindgen]
pub fn run(
    email: &str,
    brands: &str,
    format: &str,
    report: &str,
    only_flagged: &str,
    max_links: &str,
) -> Result<String, JsValue> {
    let only_flagged = matches!(
        only_flagged.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let max_links = match max_links.trim() {
        "" => 200,
        s => s
            .parse::<i64>()
            .map_err(|_| JsValue::from_str("max_links must be a whole number between 1 and 1000"))?,
    };
    gizza_ai_email_phishing_link_scanner_core::run(
        email,
        brands,
        format,
        report,
        only_flagged,
        max_links,
    )
    .map_err(|e| JsValue::from_str(&e))
}
