//! Browser-facing wasm-bindgen wrapper for /tools/contact-info-extractor/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
pub fn run(input: &str, types: &str, dedupe: &str, sort: &str) -> Result<String, JsValue> {
    let types = if types.trim().is_empty() { "both" } else { types };
    let sort = if sort.trim().is_empty() { "first-seen" } else { sort };
    gizza_ai_contact_info_extractor_core::render(input, types, truthy(dedupe), sort)
        .map_err(|e| JsValue::from_str(&e))
}
