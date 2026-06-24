//! Browser-facing wasm-bindgen wrapper for /tools/charset-transcode/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Re-decode `text` from the legacy charset `from` into clean UTF-8.
///
/// The standalone tool page passes every field value as a string:
/// - `text`: the garbled input.
/// - `from`: the source charset label (e.g. `"windows-1252"`); `"auto"` or blank
///   auto-detects.
/// - `errors`: `"replace"` (blank → replace) or `"strict"`.
/// - `passes`: a count `1`–8 (blank/unparseable → 1; the core clamps the range)
///   for un-nesting double-encoded mojibake.
///
/// Throws a JS error string on an unknown charset, a bad `errors` value, or when
/// the charset can't repair the input.
#[wasm_bindgen]
pub fn run(text: &str, from: &str, errors: &str, passes: &str) -> Result<String, JsValue> {
    let passes = passes.trim().parse::<u32>().unwrap_or(1);
    gizza_ai_charset_transcode_core::transcode(text, from, errors, passes)
        .map_err(|e| JsValue::from_str(&e))
}
