//! Browser-facing wasm-bindgen wrapper for /tools/notes-to-html-export/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    notes: &str,
    split: &str,
    toc: &str,
    toc_depth: f64,
    number_sections: &str,
    title: &str,
    theme: &str,
) -> Result<String, JsValue> {
    let depth = if toc_depth == 0.0 {
        3
    } else {
        toc_depth.round() as u32
    };
    let numbered = matches!(number_sections.trim(), "true" | "1" | "on" | "yes");
    gizza_ai_notes_to_html_export_core::export_notes(
        notes, split, toc, depth, numbered, title, theme,
    )
    .map_err(|e| JsValue::from_str(&e))
}
