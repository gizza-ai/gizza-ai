//! Browser-facing wasm-bindgen wrapper around `gizza-ai-clock-core`.

use wasm_bindgen::prelude::*;

/// Format a Unix timestamp (seconds, supplied by JS as `Date.now()/1000`) as UTC
/// RFC-3339, matching the chat skill's output exactly.
///
/// The parameter is `f64` (not `i64`) on purpose: wasm-bindgen marshals an `i64`
/// argument as a JS `BigInt`, but the page driver passes a plain JS number, so an
/// `i64` here throws "Cannot convert … to a BigInt" at call time. `f64` accepts a
/// JS number directly; whole seconds are well within f64's exact-integer range.
#[wasm_bindgen]
pub fn format_time(unix_secs: f64) -> String {
    gizza_ai_clock_core::format_secs(unix_secs as i64)
}
