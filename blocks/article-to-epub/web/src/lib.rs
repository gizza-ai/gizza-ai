//! Browser-facing wasm-bindgen wrapper for /tools/article-to-epub/.
//!
//! Packages the pasted article and returns a small JSON envelope — the book as a
//! `data:` URL plus the stats the page shows. The page's custom.js turns that
//! into a Download button and a one-line summary. The page driver passes every
//! field value as a string, so numbers and checkboxes are parsed here
//! (checkbox → `"true"`/`"false"`, blank → the descriptor default).

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_article_to_epub_core::{convert, InputFormat, Options, SplitLevel};
use wasm_bindgen::prelude::*;

const EPUB_MIME: &str = "application/epub+zip";

/// Parse a checkbox/string value as a boolean. An empty value falls back to
/// `default` (the form only sends empties before a checkbox has rendered).
fn truthy(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    content: &str,
    input_format: &str,
    title: &str,
    author: &str,
    language: &str,
    publisher: &str,
    split_level: &str,
    include_title_page: &str,
    base_font_size: &str,
) -> Result<String, JsValue> {
    // Empty input → empty result (the page renders a neutral idle state rather
    // than a red "content is empty" error on first load / after Reset).
    if content.trim().is_empty() {
        return Ok(String::new());
    }
    let base_font_size = match base_font_size.trim() {
        "" => 0.0,
        raw => raw.parse::<f64>().map_err(|_| {
            JsValue::from_str(&format!(
                "base_font_size must be a number of points between 0 and 72 (0 = the reading app's default), got {raw:?}"
            ))
        })?,
    };
    let opts = Options {
        input_format: InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?,
        title: title.to_string(),
        author: author.to_string(),
        language: language.to_string(),
        publisher: publisher.to_string(),
        split_level: SplitLevel::parse(split_level).map_err(|e| JsValue::from_str(&e))?,
        include_title_page: truthy(include_title_page, true),
        base_font_size,
    };
    let out = convert(content, &opts).map_err(|e| JsValue::from_str(&e))?;

    let envelope = serde_json::json!({
        "data_url": format!("data:{EPUB_MIME};base64,{}", B64.encode(&out.epub)),
        "filename": out.filename,
        "title": out.title,
        "chapters": out.chapters,
        "words": out.words,
        "bytes": out.epub.len(),
        "images_dropped": out.images_dropped,
        "format": out.format.name(),
    });
    Ok(envelope.to_string())
}
