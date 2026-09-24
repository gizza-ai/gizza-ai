//! Browser-facing wasm-bindgen wrapper for /tools/charcode-codec/.
//! Compiled with wasm-pack for the standalone /tools/charcode-codec/ page.
use wasm_bindgen::prelude::*;

/// Convert text to a list of character codes, or a list of codes back to text.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean param arrives as a string and is parsed here:
/// - `input`: the text to encode, or the codes to decode.
/// - `mode`: `"encode"`/`"decode"` (blank → encode).
/// - `base`: `"dec"`/`"hex"`/`"bin"`/`"oct"` (blank → dec).
/// - `scope`: `"unicode-scalar"`/`"utf8-bytes"`/`"utf16-units"`/`"ascii"`.
/// - `delimiter`: `"space"`/`"comma"`/`"newline"`/`"none"` (blank → space).
/// - `prefix`: `"none"`/`"0x"`/`"\x"`/`"U+"`.
/// - `padding`: `"none"`/`"fixed"`.
/// - `uppercase`: `"true"`/`"1"`/`"yes"`/`"on"` → uppercase hex; else lowercase.
/// - `format`: `"text"`/`"json"`.
///
/// Throws a JS error string on invalid arguments or an undecodable input.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    mode: &str,
    base: &str,
    scope: &str,
    delimiter: &str,
    prefix: &str,
    padding: &str,
    uppercase: &str,
    format: &str,
) -> Result<String, JsValue> {
    let truthy = matches!(
        uppercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_charcode_codec_core::convert(
        input, mode, base, scope, delimiter, prefix, padding, truthy, format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
