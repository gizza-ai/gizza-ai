//! Browser-facing wasm-bindgen wrapper for /tools/html-email-to-text/.
//! Field order must match page/meta.toml: html, links, wrap.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(html: &str, links: &str, wrap: f64) -> Result<String, JsValue> {
    let wrap = if wrap.is_finite() && wrap > 0.0 {
        wrap as u32
    } else {
        0
    };
    gizza_ai_html_email_to_text_core::convert(html, links, wrap).map_err(|e| JsValue::from_str(&e))
}
