//! Browser-facing wasm-bindgen wrapper for /tools/subtitle-merge/.
//! Compiled with wasm-pack for the standalone /tools/subtitle-merge/ page.
use gizza_ai_subtitle_merge_core::{merge, OutFormat};
use wasm_bindgen::prelude::*;

/// Merge the concatenated `subtitles` (files separated by a `===` line) into one.
///
/// The standalone tool page passes every field value as a string:
/// - `subtitles`: two or more `.srt`/`.vtt` files separated by a `===` line.
/// - `offset_ms`: cumulative per-file millisecond shift; blank → 0.
/// - `format`: `"auto"` (default/blank), `"srt"`, or `"vtt"`.
///
/// Throws a JS error string on a bad format / non-subtitle input.
#[wasm_bindgen]
pub fn run(subtitles: &str, offset_ms: &str, format: &str) -> Result<String, JsValue> {
    let offset: i64 = offset_ms.trim().parse().unwrap_or(0);
    let out = OutFormat::parse(format).map_err(|e| JsValue::from_str(&e))?;
    merge(subtitles, offset, out).map_err(|e| JsValue::from_str(&e))
}
