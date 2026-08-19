//! Browser-facing wasm-bindgen wrapper for /tools/code-comment-extractor/.
//! Field order MUST match page/meta.toml: code, language, output, kind,
//! strip_markers, line_numbers, min_length, docstrings. The page passes every
//! field as a string (the pure runtime does no numeric coercion), so
//! `min_length` arrives as text and is parsed here.
use gizza_ai_code_comment_extractor_core::extract;
use wasm_bindgen::prelude::*;

/// Page checkboxes marshal as "true"/"false"; accept the other positive forms too.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    code: &str,
    language: &str,
    output: &str,
    kind: &str,
    strip_markers: &str,
    line_numbers: &str,
    min_length: &str,
    docstrings: &str,
) -> Result<String, JsValue> {
    let min_length = match min_length.trim() {
        "" => 0,
        n => n
            .parse::<i64>()
            .map_err(|_| JsValue::from_str("min_length must be a whole number of characters"))?,
    };
    extract(
        code,
        language,
        output,
        kind,
        truthy(strip_markers),
        truthy(line_numbers),
        min_length,
        truthy(docstrings),
    )
    .map_err(|e| JsValue::from_str(&e))
}
