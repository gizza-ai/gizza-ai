//! Browser-facing wasm-bindgen wrapper for /tools/lua-formatter/.
//! tool.js passes every field value as a string, so each param is `&str` and
//! parsed here; the core owns the actual formatting + validation.
use gizza_ai_lua_formatter_core::{format, Indent, QuoteStyle};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    indent: &str,
    indent_char: &str,
    quote_style: &str,
) -> Result<String, JsValue> {
    let width: usize = indent.trim().parse().unwrap_or(2);
    let indent = if indent_char.eq_ignore_ascii_case("tab") {
        Indent::Tab
    } else {
        Indent::Spaces(width)
    };
    let style = if quote_style.trim().is_empty() {
        QuoteStyle::Preserve
    } else {
        QuoteStyle::parse(quote_style).map_err(|e| JsValue::from_str(&e))?
    };
    format(input, indent, style).map_err(|e| JsValue::from_str(&e))
}
