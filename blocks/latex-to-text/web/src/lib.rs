//! Browser-facing wasm-bindgen wrapper for /tools/latex-to-text/.
use wasm_bindgen::prelude::*;

fn parse_bool(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        "false" | "0" | "off" | "no" => false,
        _ => default,
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    math: &str,
    citations: &str,
    drop_environments: &str,
    keep_comments: &str,
    unicode: &str,
    body_only: &str,
    line_breaks: &str,
) -> Result<String, JsValue> {
    gizza_ai_latex_to_text_core::to_text(
        input,
        if math.trim().is_empty() {
            "remove"
        } else {
            math
        },
        if citations.trim().is_empty() {
            "drop"
        } else {
            citations
        },
        drop_environments,
        parse_bool(keep_comments, false),
        parse_bool(unicode, true),
        parse_bool(body_only, true),
        if line_breaks.trim().is_empty() {
            "paragraphs"
        } else {
            line_breaks
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
