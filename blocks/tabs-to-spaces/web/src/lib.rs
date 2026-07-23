//! Browser-facing wasm-bindgen wrapper for /tools/tabs-to-spaces/.
//! Field order MUST match meta.toml: text, direction, tab_width, scope.
//! Fields arrive as strings.
use wasm_bindgen::prelude::*;

fn parse_tab_width(s: &str) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(4);
    }
    t.parse::<u32>()
        .map_err(|_| JsValue::from_str("tab width must be a whole number"))
}

#[wasm_bindgen]
pub fn run(text: &str, direction: &str, tab_width: &str, scope: &str) -> Result<String, JsValue> {
    let tw = parse_tab_width(tab_width)?;
    gizza_ai_tabs_to_spaces_core::convert(text, direction, tw, scope)
        .map_err(|e| JsValue::from_str(&e))
}
