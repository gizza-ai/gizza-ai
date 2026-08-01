//! Browser-facing wasm-bindgen wrapper for /tools/yaml-formatter/.
//! Field order MUST match meta.toml: yaml, indent, style, sort_keys.
//! Fields arrive as strings; blanks fall back to the schema defaults
//! (indent=2, style=block, sort_keys=preserve).
use gizza_ai_yaml_formatter_core::{format_yaml, parse_sort, parse_style, Options};
use wasm_bindgen::prelude::*;

/// Spaces-per-level from the page's number field; blank/invalid → default 2,
/// clamped to the descriptor's 1..=8 range.
fn parse_indent(s: &str) -> usize {
    let t = s.trim();
    if t.is_empty() {
        return 2;
    }
    t.parse::<usize>().unwrap_or(2).clamp(1, 8)
}

#[wasm_bindgen]
pub fn run(yaml: &str, indent: &str, style: &str, sort_keys: &str) -> Result<String, JsValue> {
    let opts = Options {
        indent: parse_indent(indent),
        style: parse_style(style).map_err(|e| JsValue::from_str(&e))?,
        sort: parse_sort(sort_keys).map_err(|e| JsValue::from_str(&e))?,
    };
    format_yaml(yaml, opts).map_err(|e| JsValue::from_str(&e))
}
