//! Browser-facing wasm-bindgen wrapper for /tools/video-caption-burner/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core)
//! AND returns the extra virtual-FS files the drawtext chain needs — the
//! bundled font (`fontfile`) and one `textfile` per subtitle cue — as
//! `ArgvPlanWithInputs` so the page driver writes them into the browser
//! ffmpeg's otherwise-empty FS alongside the uploaded video.
//!
//! Field order MUST equal page/meta.toml: subtitles, position, font_size,
//! font_color, background, background_color, background_opacity, then the
//! uploaded `in_name`. Numeric fields arrive as f64 (blank → NaN → this tool's
//! default); `background` arrives as the checkbox string "true"/"false".
use gizza_ai_block_utils::{encode_b64, ArgvPlanWithInputs};
use gizza_ai_video_caption_burner_core::{plan, FONT_BYTES, FONT_FILE};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    subtitles: &str,
    position: &str,
    font_size: f64,
    font_color: &str,
    background: &str,
    background_color: &str,
    background_opacity: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let font_size = if font_size.is_finite() && font_size >= 1.0 {
        font_size as u32
    } else {
        24
    };
    let background = matches!(
        background.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let opacity = if background_opacity.is_finite() { background_opacity } else { 0.5 };

    let (argv, out_name, cue_files) = plan(
        in_name,
        subtitles,
        position,
        font_size,
        font_color,
        background,
        background_color,
        opacity,
    )
    .map_err(|e| JsValue::from_str(&e))?;

    // The browser ffmpeg FS is empty: hand it the font + each cue's text file.
    let mut inputs: Vec<(String, String)> = Vec::with_capacity(cue_files.len() + 1);
    inputs.push((FONT_FILE.to_string(), encode_b64(FONT_BYTES)));
    for (name, text) in cue_files.iter() {
        inputs.push((name.clone(), encode_b64(text.as_bytes())));
    }
    serde_wasm_bindgen::to_value(&ArgvPlanWithInputs { argv, out_name, inputs })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
