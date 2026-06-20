//! Browser-facing wasm-bindgen wrapper for /tools/url-encode/.
//! Compiled with wasm-pack for the standalone /tools/url-encode/ page.
use wasm_bindgen::prelude::*;

/// Percent-encode or percent-decode `text`.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean/integer params arrive as strings and are parsed here:
/// - `mode`: `"encode"`/`"decode"` (blank → encode).
/// - `target`: `"component"`/`"uri"`/`"form"` (blank → component).
/// - `per_line`: `"true"`/`"1"`/`"yes"`/`"on"` → process each line separately;
///   anything else (including blank) → off.
/// - `repeat`: a count `1`–16 (blank/unparseable → 1; the core clamps the range).
///
/// Throws a JS error string on an invalid `mode`/`target` or an invalid-UTF-8 decode.
#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    target: &str,
    per_line: &str,
    repeat: &str,
) -> Result<String, JsValue> {
    let per_line = matches!(
        per_line.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let repeat = repeat.trim().parse::<u32>().unwrap_or(1);
    gizza_ai_url_encode_core::convert(text, mode, target, per_line, repeat)
        .map_err(|e| JsValue::from_str(&e))
}
