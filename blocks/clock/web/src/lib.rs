//! Browser-facing wasm-bindgen wrapper around `gizza-ai-clock-core`.

use wasm_bindgen::prelude::*;

/// Format a Unix timestamp (seconds, supplied by JS as Date.now()/1000) as UTC
/// RFC-3339, matching the chat skill's output exactly.
#[wasm_bindgen]
pub fn format_time(unix_secs: i64) -> String {
    gizza_ai_clock_core::format_secs(unix_secs)
}
