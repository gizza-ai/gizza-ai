//! Browser-facing wasm-bindgen wrapper for /tools/html-extract/.
//! Field order MUST match meta.toml: html, selector, extract, attr, limit, trim.
use gizza_ai_html_extract_core::{extract, parse_extract};
use wasm_bindgen::prelude::*;

fn truthy_default_true(s: &str) -> bool {
    !matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

#[wasm_bindgen]
pub fn run(
    html: &str,
    selector: &str,
    extract_mode: &str,
    attr: &str,
    limit: &str,
    trim: &str,
) -> Result<String, JsValue> {
    let mode = parse_extract(extract_mode).map_err(|e| JsValue::from_str(&e))?;
    let lim: usize = limit.trim().parse().unwrap_or(100);
    extract(html, selector, mode, attr, lim, truthy_default_true(trim))
        .map_err(|e| JsValue::from_str(&e))
}
