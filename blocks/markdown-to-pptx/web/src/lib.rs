//! Browser-facing wasm-bindgen wrapper for /tools/markdown-to-pptx/.
//!
//! Parses the Markdown outline and returns the `.pptx` deck as a `data:` URL
//! string; the page's custom.js turns that into a Download button. Every
//! argument arrives from the form as a string.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_markdown_to_pptx_core::{to_pptx, AspectRatio, SplitLevel, Theme};
use wasm_bindgen::prelude::*;

const PPTX_MIME: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    title: &str,
    split_level: &str,
    theme: &str,
    aspect_ratio: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page renders a neutral idle state rather
    // than a red error on first load / after Reset).
    if markdown.trim().is_empty() && title.trim().is_empty() {
        return Ok(String::new());
    }
    let split = SplitLevel::parse(split_level).map_err(|e| JsValue::from_str(&e))?;
    let th = Theme::parse(theme).map_err(|e| JsValue::from_str(&e))?;
    let aspect = AspectRatio::parse(aspect_ratio).map_err(|e| JsValue::from_str(&e))?;
    let bytes = to_pptx(markdown, title, split, th, aspect).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("data:{PPTX_MIME};base64,{}", B64.encode(&bytes)))
}
