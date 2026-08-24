//! Browser-facing wasm-bindgen wrapper for /tools/markdown-list-marker-normalizer/.
use wasm_bindgen::prelude::*;

fn int(v: &str, fallback: i64, field: &str) -> Result<i64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<i64>()
        .map_err(|_| format!("{field} must be a whole number, got '{v}'"))
}

fn flag(v: &str, fallback: bool) -> bool {
    let t = v.trim().to_ascii_lowercase();
    if t.is_empty() {
        fallback
    } else {
        matches!(t.as_str(), "true" | "1" | "yes" | "on")
    }
}

fn word(v: &str, fallback: &str) -> String {
    let t = v.trim();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    marker: &str,
    indent: &str,
    normalize_indent: &str,
    ordered: &str,
    marker_space: &str,
) -> Result<String, JsValue> {
    let indent = int(indent, 2, "indent").map_err(|e| JsValue::from_str(&e))?;
    let marker_space = int(marker_space, 1, "marker_space").map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_markdown_list_marker_normalizer_core::normalize(
        markdown,
        &word(marker, "dash"),
        indent,
        flag(normalize_indent, true),
        &word(ordered, "keep"),
        marker_space,
    )
    .map_err(|e| JsValue::from_str(&e))
}
