//! Browser-facing wasm-bindgen wrapper for /tools/video-gps-metadata-extractor/.
//! Compiled with wasm-pack for the standalone tool page.
use wasm_bindgen::prelude::*;

/// Extract embedded GPS location tags from an MP4/MOV/QuickTime video.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the video file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"`. Empty is treated as base64.
/// - `output`: `"report"` (default, human-readable) or `"json"` (structured).
///
/// Throws a JS error string on undecodable input, bytes that are not a
/// recognizable container, or an invalid `input_format`/`output` value.
#[wasm_bindgen]
pub fn run(input: &str, input_format: &str, output: &str) -> Result<String, JsValue> {
    gizza_ai_video_gps_metadata_extractor_core::run(input, input_format, output)
        .map_err(|e| JsValue::from_str(&e))
}
