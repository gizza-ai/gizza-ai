//! Browser-facing wasm-bindgen wrapper for /tools/html-sanitizer/.
//! Compiled with wasm-pack for the standalone page. Field order MUST match
//! page/meta.toml: html, mode, allow_links, allow_images, allow_styles,
//! keep_classes, keep_comments.
use wasm_bindgen::prelude::*;

fn parse_bool(value: &str, default: bool) -> bool {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        default
    } else {
        matches!(v.as_str(), "true" | "1" | "yes" | "on")
    }
}

/// Sanitize `html` and render safe HTML markup or plain text.
///
/// The standalone tool page passes every field value as a string; checkbox
/// strings are parsed here so the wasm export stays compatible with the shared
/// page runtime.
#[wasm_bindgen]
pub fn run(
    html: &str,
    mode: &str,
    allow_links: &str,
    allow_images: &str,
    allow_styles: &str,
    keep_classes: &str,
    keep_comments: &str,
) -> Result<String, JsValue> {
    gizza_ai_html_sanitizer_core::render(
        html,
        mode,
        parse_bool(allow_links, true),
        parse_bool(allow_images, true),
        parse_bool(allow_styles, false),
        parse_bool(keep_classes, true),
        parse_bool(keep_comments, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
