//! Browser-facing wasm-bindgen wrapper for /tools/gmail-takeout-parser/.
//! Field order MUST match meta.toml: input, format, include_body, snippet_chars.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    format: &str,
    include_body: &str,
    snippet_chars: &str,
) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "csv" } else { format };
    let chars = snippet_chars.trim().parse::<usize>().unwrap_or(200);
    gizza_ai_gmail_takeout_parser_core::run(input, format, truthy(include_body), chars)
        .map_err(|e| JsValue::from_str(&e))
}
