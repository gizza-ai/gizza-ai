//! Browser-facing wasm-bindgen wrapper for /tools/srt-to-vtt/.
//! Compiled with wasm-pack for the standalone /tools/srt-to-vtt/ page.
use gizza_ai_srt_to_vtt_core::{auto_convert, convert, Direction};
use wasm_bindgen::prelude::*;

/// Convert subtitle text between SubRip (.srt) and WebVTT (.vtt).
///
/// The standalone tool page passes every field value as a string:
/// - `subtitles`: the full .srt or .vtt subtitle text.
/// - `direction`: `"auto"` (default/blank), `"srt-to-vtt"`, or `"vtt-to-srt"`.
///
/// Throws a JS error string on a bad direction / non-subtitle input / malformed
/// timing line.
#[wasm_bindgen]
pub fn run(subtitles: &str, direction: &str) -> Result<String, JsValue> {
    let result = match direction.trim() {
        "" | "auto" => auto_convert(subtitles),
        "srt-to-vtt" => convert(subtitles, Direction::SrtToVtt),
        "vtt-to-srt" => convert(subtitles, Direction::VttToSrt),
        other => Err(format!(
            "invalid direction {other:?}: expected \"auto\", \"srt-to-vtt\", or \"vtt-to-srt\""
        )),
    };
    result.map_err(|e| JsValue::from_str(&e))
}
