//! Browser-facing wasm-bindgen wrapper for /tools/video-title-card/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core)
//! AND returns the extra virtual-FS files the drawtext filter needs — the
//! bundled font (`fontfile`) and the overlay text (`textfile`) — as
//! `ArgvPlanWithInputs` so the page driver writes them into the browser
//! ffmpeg's otherwise-empty FS alongside the uploaded video.
//!
//! Field order MUST equal page/meta.toml: text, position, font_size,
//! font_color, background, background_color, background_opacity, start, end,
//! then the uploaded `in_name`. Numeric fields arrive as f64 (blank → NaN →
//! this tool's default); `background` arrives as the checkbox string
//! "true"/"false".
use gizza_ai_block_utils::{encode_b64, ArgvPlanWithInputs};
use gizza_ai_video_title_card_core::{plan, FONT_BYTES, FONT_FILE, TEXT_FILE};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    text: &str,
    position: &str,
    font_size: f64,
    font_color: &str,
    background: &str,
    background_color: &str,
    background_opacity: f64,
    start: f64,
    end: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let font_size = if font_size.is_finite() && font_size >= 1.0 {
        font_size as u32
    } else {
        48
    };
    let background = matches!(
        background.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let opacity = if background_opacity.is_finite() { background_opacity } else { 0.5 };
    let start = if start.is_finite() { start } else { 0.0 };
    let end = if end.is_finite() { end } else { 5.0 };

    let (argv, out_name) = plan(
        in_name,
        text,
        position,
        font_size,
        font_color,
        background,
        background_color,
        opacity,
        start,
        end,
    )
    .map_err(|e| JsValue::from_str(&e))?;

    // The browser ffmpeg FS is empty: hand it the font + the (validated) text.
    let inputs = vec![
        (FONT_FILE.to_string(), encode_b64(FONT_BYTES)),
        (TEXT_FILE.to_string(), encode_b64(text.as_bytes())),
    ];
    serde_wasm_bindgen::to_value(&ArgvPlanWithInputs { argv, out_name, inputs })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
