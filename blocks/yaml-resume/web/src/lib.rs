//! Browser-facing wasm-bindgen wrapper for /tools/yaml-resume/.
//! The standalone page passes every field value as a string (in the
//! `page/meta.toml` `[[input]]` order, which mirrors the descriptor's param
//! order), so the two numeric sliders arrive as strings and are parsed here.
//! Enum `<select>`s always send a value, but a blank falls back to the
//! descriptor default so a cleared field never errors.
use gizza_ai_yaml_resume_core::{
    build, sanitize_accent, sanitize_font_size, sanitize_margin, sanitize_sections, DateFormat,
    Font, InputFormat, Options, PageSize, Theme,
};
use wasm_bindgen::prelude::*;

fn or<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

/// Blank or unparseable → the descriptor default, so a cleared slider renders
/// rather than erroring; a real out-of-range value still fails validation.
fn num(v: &str, default: f64) -> f64 {
    v.trim().parse::<f64>().unwrap_or(default)
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    format: &str,
    theme: &str,
    accent: &str,
    font: &str,
    font_size: &str,
    page_size: &str,
    margin: &str,
    date_format: &str,
    sections: &str,
) -> Result<String, JsValue> {
    let js = |e: String| JsValue::from_str(&e);
    let opts = Options {
        format: InputFormat::parse(or(format, "auto")).map_err(js)?,
        theme: Theme::parse(or(theme, "modern")).map_err(js)?,
        date_format: DateFormat::parse(or(date_format, "month-year")).map_err(js)?,
        accent: sanitize_accent(accent).map_err(js)?,
        font: Font::parse(or(font, "sans")).map_err(js)?,
        font_size: sanitize_font_size(num(font_size, 10.5)).map_err(js)?,
        page_size: PageSize::parse(or(page_size, "letter")).map_err(js)?,
        margin: sanitize_margin(num(margin, 0.5)).map_err(js)?,
        sections: sanitize_sections(sections).map_err(js)?,
    };
    build(data, &opts).map_err(js)
}
