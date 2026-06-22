//! Browser-facing wasm-bindgen wrapper for /tools/sitemap-url-extractor/.
//! Single field: xml. Renders one URL per line (with a tab-separated lastmod
//! column when present).
use gizza_ai_sitemap_url_extractor_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(xml: &str) -> Result<String, JsValue> {
    render(xml).map_err(|e| JsValue::from_str(&e))
}
