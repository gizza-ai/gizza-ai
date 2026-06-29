//! Browser-facing wasm-bindgen wrapper for /tools/tail-lines/.
//! Compiled with wasm-pack for the standalone /tools/tail-lines/ page.
use wasm_bindgen::prelude::*;

/// Output the last `count` lines of `text`.
///
/// The standalone tool page passes every field value as a string, so the
/// integer/boolean params arrive as strings and are parsed here:
/// - `count`: how many trailing lines to keep (blank/unparseable → 0, which the
///   core maps to its default of 10; the core also clamps the range).
/// - `skip`: lines to drop from the end first (blank/unparseable → 0).
/// - `number`: `"true"`/`"1"`/`"on"`/`"yes"` → prefix line numbers; anything
///   else (including blank) → off.
#[wasm_bindgen]
pub fn run(text: &str, count: &str, skip: &str, number: &str) -> Result<String, JsValue> {
    let count = count.trim().parse::<u32>().unwrap_or(0);
    let skip = skip.trim().parse::<u32>().unwrap_or(0);
    let number = matches!(
        number.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_tail_lines_core::tail(text, count, skip, number).map_err(|e| JsValue::from_str(&e))
}
