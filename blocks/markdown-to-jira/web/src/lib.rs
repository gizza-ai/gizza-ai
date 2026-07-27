//! Browser-facing wasm-bindgen wrapper for /tools/markdown-to-jira/.
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_i64(s: &str, default: i64) -> i64 {
    if s.trim().is_empty() {
        default
    } else {
        s.trim().parse().unwrap_or(default)
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    direction: &str,
    heading_offset: &str,
    panel_blockquotes: &str,
) -> Result<String, JsValue> {
    let heading_offset = parse_i64(heading_offset, 0);
    let panel_blockquotes = parse_bool(panel_blockquotes, true);
    gizza_ai_markdown_to_jira_core::convert(input, direction, heading_offset, panel_blockquotes)
        .map_err(|e| JsValue::from_str(&e))
}
