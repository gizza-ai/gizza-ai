//! Browser-facing wasm-bindgen wrapper for /tools/persian-tokenizer/.
//! Compiled with wasm-pack for the standalone /tools/persian-tokenizer/ page.
use wasm_bindgen::prelude::*;

/// Tokenize `text` into Persian words and/or sentences and render the result.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as text and are parsed here, each falling back to the
/// descriptor's default when the field is left blank:
/// - `mode`: `"words"` (default) / `"sentences"` / `"both"`.
/// - `format`: `"lines"` (default) / `"numbered"` / `"space-separated"` / `"json"`.
/// - `punctuation`: `"separate"` (default) / `"attach"` / `"remove"`.
/// - `split_zwnj`: `"true"`/`"1"`/`"yes"`/`"on"` → break `می‌خوانیم` into
///   `می` + `خوانیم`; blank → the default (off).
/// - `normalize`: default ON — fold Arabic letters, strip harakat/kashida.
/// - `keep_entities`: default ON — URLs, emails, @mentions, #hashtags and
///   separator-bearing numbers stay one token.
/// - `newlines`: `"paragraph"` (default) / `"never"` / `"always"`.
///
/// Throws a JS error string on an unknown option value, on empty or over-long
/// input, and when the options leave no tokens at all.
#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    format: &str,
    punctuation: &str,
    split_zwnj: &str,
    normalize: &str,
    keep_entities: &str,
    newlines: &str,
) -> Result<String, JsValue> {
    // Blank selects fall through to the core's own "" → default handling.
    let split_zwnj =
        parse_bool(split_zwnj, "split_zwnj", false).map_err(|e| JsValue::from_str(&e))?;
    let normalize = parse_bool(normalize, "normalize", true).map_err(|e| JsValue::from_str(&e))?;
    let keep_entities =
        parse_bool(keep_entities, "keep_entities", true).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_persian_tokenizer_core::run(
        text,
        mode,
        format,
        punctuation,
        split_zwnj,
        normalize,
        keep_entities,
        newlines,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Checkbox / query-param booleans arrive as text; anything unrecognised is a
/// hard error rather than a silent `false`.
fn parse_bool(v: &str, name: &str, default: bool) -> Result<bool, String> {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => Ok(default),
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!(
            "invalid {name} {other:?}: expected \"true\" or \"false\""
        )),
    }
}
