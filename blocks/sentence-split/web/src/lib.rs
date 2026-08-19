//! Browser-facing wasm-bindgen wrapper for /tools/sentence-split/.
//! Compiled with wasm-pack for the standalone /tools/sentence-split/ page.
use wasm_bindgen::prelude::*;

/// Split `text` into sentences and render the result.
///
/// The standalone tool page passes every field value as a string, so the
/// non-string params arrive as strings and are parsed here, each falling back
/// to the descriptor's default when the field is left blank:
/// - `format`: `"lines"` (default) / `"numbered"` / `"blank-line"` / `"json"`.
/// - `newlines`: `"paragraph"` (default) / `"never"` / `"always"`.
/// - `trim`: `"true"`/`"1"`/`"yes"`/`"on"` → trim and fold inner newlines;
///   `"false"`/`"0"`/`"no"`/`"off"` → preserve spacing inside each sentence;
///   blank → the default (on). The page renders it as a checkbox.
/// - `min_chars`: a non-negative integer; blank → 0 (keep every sentence).
/// - `extra_abbreviations`: free text, blank → none.
///
/// Throws a JS error string on an invalid `format`/`newlines`/`min_chars`, on
/// empty or over-long input, or when `min_chars` filters every sentence away.
#[wasm_bindgen]
pub fn run(
    text: &str,
    format: &str,
    newlines: &str,
    trim: &str,
    min_chars: &str,
    extra_abbreviations: &str,
) -> Result<String, JsValue> {
    // Blank selects fall through to the core's own "" → default handling.
    let trim = parse_bool(trim, true).map_err(|e| JsValue::from_str(&e))?;
    let min_chars = parse_min_chars(min_chars).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_sentence_split_core::run(text, format, newlines, trim, min_chars, extra_abbreviations)
        .map_err(|e| JsValue::from_str(&e))
}

/// Checkbox / query-param booleans arrive as text; anything unrecognised is a
/// hard error rather than a silent `false`.
fn parse_bool(v: &str, default: bool) -> Result<bool, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => Ok(default),
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!(
            "invalid trim {other:?}: expected \"true\" or \"false\""
        )),
    }
}

fn parse_min_chars(v: &str) -> Result<usize, String> {
    let v = v.trim();
    if v.is_empty() {
        return Ok(0);
    }
    v.parse::<usize>().map_err(|_| {
        format!(
            "invalid min_chars {v:?}: expected a whole number from 0 to {}",
            gizza_ai_sentence_split_core::MAX_MIN_CHARS
        )
    })
}
