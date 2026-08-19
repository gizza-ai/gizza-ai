//! Browser-facing wasm-bindgen wrapper for /tools/email-tracker-pixel-detector/.
//! Field order MUST match meta.toml: text, format, report, include_links, vendors.

use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    format: &str,
    report: &str,
    include_links: &str,
    vendors: &str,
) -> Result<String, JsValue> {
    gizza_ai_email_tracker_pixel_detector_core::run(
        text,
        format,
        report,
        truthy(include_links),
        vendors,
    )
    .map_err(|e| JsValue::from_str(&e))
}
