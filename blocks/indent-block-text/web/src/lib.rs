//! Browser-facing wasm-bindgen wrapper for /tools/indent-block-text/.
use wasm_bindgen::prelude::*;

fn int(v: &str, fallback: i64) -> Result<i64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<i64>()
        .map_err(|_| format!("count must be a whole number, got '{v}'"))
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
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    mode: &str,
    style: &str,
    count: &str,
    prefix: &str,
    lines: &str,
    skip_blank_lines: &str,
) -> Result<String, JsValue> {
    gizza_ai_indent_block_text_core::run_with_options(
        text,
        &word(mode, "indent"),
        &word(style, "spaces"),
        int(count, 4).map_err(|e| JsValue::from_str(&e))?,
        prefix,
        &word(lines, "all"),
        flag(skip_blank_lines, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
