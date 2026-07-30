//! Browser-facing wasm-bindgen wrapper for /tools/format-css/.
//! Field order MUST match meta.toml: input, indent, indent_char, sort,
//! selectors_per_line, uppercase_hex. Fields arrive as strings; booleans as
//! "true"/"false" (blank falls back to the schema defaults).
use gizza_ai_format_css_core::{format, parse_sort, Indent, Options};
use wasm_bindgen::prelude::*;

/// Spaces-per-level from the page's number field; blank/invalid → default 2,
/// clamped to the descriptor's 0..=8 range.
fn parse_indent(s: &str) -> usize {
    let t = s.trim();
    if t.is_empty() {
        return 2;
    }
    t.parse::<usize>().unwrap_or(2).min(8)
}

/// Positive-truthy checkbox parse (`"true"`/`"1"`/`"on"`/`"yes"`).
fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    indent: &str,
    indent_char: &str,
    sort: &str,
    selectors_per_line: &str,
    uppercase_hex: &str,
) -> Result<String, JsValue> {
    let indent = if indent_char.eq_ignore_ascii_case("tab") {
        Indent::Tab
    } else {
        Indent::Spaces(parse_indent(indent))
    };
    let sort_name = if sort.trim().is_empty() { "none" } else { sort };
    let sort = parse_sort(sort_name)
        .ok_or_else(|| JsValue::from_str(&format!("unknown sort '{sort_name}'")))?;
    let opts = Options {
        indent,
        sort,
        // Default-true checkbox: a blank string only happens if the field is
        // absent — treat that as the default (on).
        selectors_per_line: selectors_per_line.trim().is_empty() || truthy(selectors_per_line),
        uppercase_hex: truthy(uppercase_hex),
    };
    format(input, opts).map_err(|e| JsValue::from_str(&e))
}
