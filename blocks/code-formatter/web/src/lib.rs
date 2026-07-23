//! Browser-facing wasm-bindgen wrapper for /tools/code-formatter/.
//! Field order MUST match meta.toml: code, language, indent, indent_char.
//! Fields arrive as strings; blanks fall back to the schema defaults
//! (language=auto, indent=2, indent_char=space).
use gizza_ai_code_formatter_core::{detect_language, format, parse_language, Indent};
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
pub fn run(code: &str, language: &str, indent: &str, indent_char: &str) -> Result<String, JsValue> {
    let language = if language.trim().is_empty() {
        "auto"
    } else {
        language
    };
    let lang = if language.eq_ignore_ascii_case("auto") {
        detect_language(code)
    } else {
        parse_language(language)
            .ok_or_else(|| JsValue::from_str(&format!("unknown language '{language}'")))?
    };
    let ind = if indent_char.eq_ignore_ascii_case("tab") {
        Indent::Tab
    } else {
        Indent::Spaces(parse_indent(indent))
    };
    format(code, lang, ind).map_err(|e| JsValue::from_str(&e))
}
