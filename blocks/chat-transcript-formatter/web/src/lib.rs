//! Browser-facing wasm-bindgen wrapper for /tools/chat-transcript-formatter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    output_format: &str,
    time_format: &str,
    include_dates: &str,
    merge_consecutive: &str,
    blank_line_between: &str,
) -> Result<String, JsValue> {
    gizza_ai_chat_transcript_formatter_core::run(
        input,
        output_format,
        time_format,
        truthy(include_dates),
        truthy(merge_consecutive),
        truthy(blank_line_between),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
