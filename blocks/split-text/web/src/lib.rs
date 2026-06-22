//! Browser-facing wasm-bindgen wrapper for /tools/split-text/.
//! Compiled with wasm-pack for the standalone /tools/split-text/ page.
use wasm_bindgen::prelude::*;

/// Split `text` on a delimiter into one item per line.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as strings and are parsed here:
/// - `delimiter`: the substring to split on (blank → the core's literal-mode
///   empty-delimiter error; use mode whitespace/chars instead). Escapes \n \t \r \\.
/// - `mode`: `"literal"`/`"whitespace"`/`"chars"` (blank → literal).
/// - `trim` / `remove_empty`: `"true"`/`"1"`/`"yes"`/`"on"` → on; else off.
///
/// Throws a JS error string on an invalid `mode` or an empty literal delimiter.
#[wasm_bindgen]
pub fn run(
    text: &str,
    delimiter: &str,
    mode: &str,
    trim: &str,
    remove_empty: &str,
) -> Result<String, JsValue> {
    let trim = is_truthy(trim);
    let remove_empty = is_truthy(remove_empty);
    // The page renders `delimiter` as a plain text field that starts empty; the
    // schema default (",") is not pre-filled into it. So when the field is blank
    // AND we're in literal mode (where a delimiter is required), fall back to the
    // comma default rather than erroring. Whitespace/chars modes ignore it.
    let is_literal = matches!(mode.trim(), "" | "literal");
    let delimiter = if is_literal && delimiter.is_empty() {
        ","
    } else {
        delimiter
    };
    gizza_ai_split_text_core::split(text, delimiter, mode, trim, remove_empty)
        .map_err(|e| JsValue::from_str(&e))
}

fn is_truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
