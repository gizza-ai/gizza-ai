//! Browser-facing wasm-bindgen wrapper for /tools/markdown-deck-to-pdf/.
//!
//! Renders the Markdown deck and returns the PDF as a `data:` URL string; the
//! page's custom.js turns that into a Download button. Every argument arrives
//! from the form as a string, in the `page/meta.toml` `[[input]]` order.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_markdown_deck_to_pdf_core::{
    to_pdf, DeckOptions, SlideSize, SplitLevel, Theme, DEFAULT_FONT_SIZE,
};
use wasm_bindgen::prelude::*;

const PDF_MIME: &str = "application/pdf";

/// Parse a checkbox field: the page marshals booleans as "true"/"false".
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    markdown: &str,
    title: &str,
    split_level: &str,
    slide_size: &str,
    theme: &str,
    font_size: &str,
    header: &str,
    footer: &str,
    page_numbers: &str,
    outline: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page renders a neutral idle state rather
    // than a red error on first load / after Reset).
    if markdown.trim().is_empty() && title.trim().is_empty() {
        return Ok(String::new());
    }
    let split = SplitLevel::parse(split_level).map_err(|e| JsValue::from_str(&e))?;
    let size = SlideSize::parse(slide_size).map_err(|e| JsValue::from_str(&e))?;
    let th = Theme::parse(theme).map_err(|e| JsValue::from_str(&e))?;
    let fs = if font_size.trim().is_empty() {
        DEFAULT_FONT_SIZE
    } else {
        font_size
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str("font_size must be a number between 8 and 48"))?
    };

    let opts = DeckOptions {
        title,
        split,
        size,
        theme: th,
        font_size: fs,
        header,
        footer,
        page_numbers: truthy(page_numbers),
        outline: truthy(outline),
    };
    let bytes = to_pdf(markdown, &opts).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("data:{PDF_MIME};base64,{}", B64.encode(&bytes)))
}
