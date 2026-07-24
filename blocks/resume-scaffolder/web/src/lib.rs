//! Browser-facing wasm-bindgen wrapper for /tools/resume-scaffolder/.
//! The standalone page passes every field value as a string; enum `<select>`s
//! always send a value, but blank falls back to the descriptor default so a
//! cleared field never errors.
use gizza_ai_resume_scaffolder_core::{build, sanitize_accent, Font, Options, PageSize, Theme};
use wasm_bindgen::prelude::*;

fn or<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    theme: &str,
    accent: &str,
    font: &str,
    page_size: &str,
) -> Result<String, JsValue> {
    let js = |e: String| JsValue::from_str(&e);
    let opts = Options {
        theme: Theme::parse(or(theme, "modern")).map_err(js)?,
        accent: sanitize_accent(accent).map_err(js)?,
        font: Font::parse(or(font, "sans")).map_err(js)?,
        page_size: PageSize::parse(or(page_size, "letter")).map_err(js)?,
    };
    build(data, &opts).map_err(js)
}
