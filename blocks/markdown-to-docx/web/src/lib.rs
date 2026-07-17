//! Browser-facing wasm-bindgen wrapper for /tools/markdown-to-docx/.
//!
//! Returns the generated `.docx` as a `data:` URL; page/custom.js renders that
//! value as a Download button instead of dumping base64 into the text panel.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_markdown_to_docx_core::{to_docx, FontFamily, PageSize, MAX_FONT_SIZE, MIN_FONT_SIZE};
use wasm_bindgen::prelude::*;

const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    title: &str,
    page_size: &str,
    font_family: &str,
    font_size: &str,
    page_break: &str,
) -> Result<String, JsValue> {
    if markdown.trim().is_empty() && title.trim().is_empty() {
        return Ok(String::new());
    }
    let page_size = PageSize::parse(page_size).map_err(|e| JsValue::from_str(&e))?;
    let font_family = FontFamily::parse(font_family).map_err(|e| JsValue::from_str(&e))?;
    let font_size: f64 = font_size
        .trim()
        .parse()
        .map_err(|_| JsValue::from_str("font_size must be a number"))?;
    if !font_size.is_finite() || font_size < MIN_FONT_SIZE as f64 || font_size > MAX_FONT_SIZE as f64 {
        return Err(JsValue::from_str(&format!(
            "font_size must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE}"
        )));
    }
    let page_break = matches!(
        page_break.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let bytes = to_docx(
        markdown,
        title,
        page_size,
        font_family,
        font_size.round() as u32,
        page_break,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(format!("data:{DOCX_MIME};base64,{}", B64.encode(&bytes)))
}
