//! Browser-facing wasm-bindgen wrapper for /tools/youtube-thumbnail-generator/.
//! Returns ffmpeg argv plus the bundled font and headline text as extra virtual
//! FS inputs for the browser ffmpeg bridge.

use gizza_ai_block_utils::{encode_b64, ArgvPlanWithInputs};
use gizza_ai_youtube_thumbnail_generator_core::{plan, FONT_BYTES, FONT_FILE, TEXT_FILE};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    headline: &str,
    timestamp: f64,
    width: f64,
    height: f64,
    text_position: &str,
    font_size: f64,
    text_color: &str,
    outline_color: &str,
    accent_color: &str,
    accent_position: &str,
    accent_size: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(
        in_name,
        headline,
        if timestamp.is_finite() { timestamp } else { 0.0 },
        if width.is_finite() { width } else { 1280.0 },
        if height.is_finite() { height } else { 720.0 },
        text_position,
        if font_size.is_finite() { font_size } else { 96.0 },
        text_color,
        outline_color,
        accent_color,
        accent_position,
        if accent_size.is_finite() { accent_size } else { 22.0 },
    )
    .map_err(|e| JsValue::from_str(&e))?;
    let inputs = vec![
        (FONT_FILE.to_string(), encode_b64(FONT_BYTES)),
        (TEXT_FILE.to_string(), encode_b64(headline.as_bytes())),
    ];
    serde_wasm_bindgen::to_value(&ArgvPlanWithInputs { argv, out_name, inputs })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
