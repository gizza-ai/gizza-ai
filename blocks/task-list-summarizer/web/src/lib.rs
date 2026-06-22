//! Browser-facing wasm-bindgen wrapper for /tools/task-list-summarizer/.
//! Compiled with wasm-pack for the standalone /tools/task-list-summarizer/ page.
use wasm_bindgen::prelude::*;

/// Summarize a Markdown task list.
///
/// The standalone tool page passes every field value as a string:
/// - `text`: the Markdown checklist.
/// - `mode`: `"summary"`/`"done"`/`"pending"`/`"json"` (blank → summary).
///
/// Throws a JS error string on an invalid `mode`.
#[wasm_bindgen]
pub fn run(text: &str, mode: &str) -> Result<String, JsValue> {
    gizza_ai_task_list_summarizer_core::summarize(text, mode).map_err(|e| JsValue::from_str(&e))
}
