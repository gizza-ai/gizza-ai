//! Browser-facing wasm-bindgen wrapper for /tools/markdown-to-slides/.
//! Compiled with wasm-pack for the standalone /tools/markdown-to-slides/ page.
use gizza_ai_markdown_to_slides_core::build_slides;
use wasm_bindgen::prelude::*;

/// Build a self-contained HTML slide deck from `markdown`. `theme` is the
/// `<select>` value ("light"|"dark"; blank → "light"); `title` is the optional
/// document title. Throws a JS error string on empty input / unknown theme.
#[wasm_bindgen]
pub fn run(markdown: &str, theme: &str, title: &str) -> Result<String, JsValue> {
    let theme = if theme.is_empty() { "light" } else { theme };
    build_slides(markdown, theme, title).map_err(|e| JsValue::from_str(&e))
}
