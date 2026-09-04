//! Browser-facing wasm-bindgen wrapper for /tools/dockerfile-formatter/.
//! The page hands every field over as a string, so booleans and numbers are
//! parsed here and empty fields fall back to the descriptor defaults.
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_usize(name: &str, value: &str, default: usize) -> Result<usize, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<usize>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    instruction_case: &str,
    indent: &str,
    align_continuations: &str,
    max_blank_lines: &str,
    blank_line_between_stages: &str,
    normalize_comments: &str,
) -> Result<String, JsValue> {
    gizza_ai_dockerfile_formatter_core::run(
        input,
        instruction_case,
        parse_usize("indent", indent, 4)?,
        truthy(align_continuations, false),
        parse_usize("max_blank_lines", max_blank_lines, 1)?,
        truthy(blank_line_between_stages, true),
        truthy(normalize_comments, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
