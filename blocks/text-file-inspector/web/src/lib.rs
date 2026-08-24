//! Browser-facing wasm-bindgen wrapper for /tools/text-file-inspector/.
//!
//! The page accepts pasted text in a textarea. Chat/CLI can inspect exact bytes
//! via base64 or hex; the browser page intentionally uses literal text because
//! the generic pure-tool runtime does not expose a file-upload-to-base64 bridge.
//! Argument order MUST match the `[[input]]` order in page/meta.toml: input,
//! output, longest_lines, max_line_length, preview_lines. Fields arrive as strings.
use wasm_bindgen::prelude::*;

fn parse_int(s: &str, field: &str, default: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    output: &str,
    longest_lines: &str,
    max_line_length: &str,
    preview_lines: &str,
) -> Result<String, JsValue> {
    // No file chosen yet (or Reset): show an empty widget, not an error.
    if input.trim().is_empty() {
        return Ok(String::new());
    }
    let out = if output.trim().is_empty() {
        "report"
    } else {
        output
    };
    gizza_ai_text_file_inspector_core::run(
        input,
        "text",
        out,
        parse_int(longest_lines, "longest_lines", 3)?,
        parse_int(max_line_length, "max_line_length", 0)?,
        parse_int(preview_lines, "preview_lines", 0)?,
    )
    .map_err(|e| JsValue::from_str(&e))
}
