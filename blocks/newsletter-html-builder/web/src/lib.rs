//! Browser-facing wasm-bindgen wrapper for /tools/newsletter-html-builder/.
//! Field values arrive as strings (checkboxes as "true"/"false"). Keep `width`
//! stringly here too: the generic pure-tool driver passes field values as text
//! and leaves validation/defaulting to the tool.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    sections: &str,
    subject: &str,
    preheader: &str,
    width: &str,
    background: &str,
    content_background: &str,
    text_color: &str,
    accent: &str,
    font: &str,
    dark_mode: &str,
) -> Result<String, JsValue> {
    let dark = matches!(dark_mode.trim(), "true" | "1" | "on" | "yes");
    let width_px = if width.trim().is_empty() {
        0.0
    } else {
        width
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str("invalid width: expected a whole number of pixels"))?
    };
    gizza_ai_newsletter_html_builder_core::build(
        sections,
        subject,
        preheader,
        width_px,
        background,
        content_background,
        text_color,
        accent,
        font,
        dark,
    )
    .map_err(|e| JsValue::from_str(&e))
}
