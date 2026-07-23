//! Browser-facing wasm-bindgen wrapper for /tools/video-ken-burns/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! The page driver calls `build_argv(...fieldArgs, in_name)`, so the field order
//! here MUST equal the `[[input]]` field order in `page/meta.toml`
//! (direction, duration, zoom, width, height, fps). Empty numeric fields arrive
//! as `0` (JS `Number("")` → 0), so a `0`/blank value means "use the default" —
//! the same contract the chat block applies in `resolved()`.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

// Defaults mirror the chat block's DEF_* constants so every surface agrees.
const DEF_DURATION: f64 = 5.0;
const DEF_ZOOM: f64 = 1.3;
const DEF_WIDTH: u32 = 1280;
const DEF_HEIGHT: u32 = 720;
const DEF_FPS: f64 = 30.0;

#[wasm_bindgen]
pub fn build_argv(
    direction: &str,
    duration: f64,
    zoom: f64,
    width: f64,
    height: f64,
    fps: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let duration = if duration > 0.0 { duration } else { DEF_DURATION };
    let zoom = if zoom > 0.0 { zoom } else { DEF_ZOOM };
    let width = if width > 0.0 { width.round() as u32 } else { DEF_WIDTH };
    let height = if height > 0.0 { height.round() as u32 } else { DEF_HEIGHT };
    let fps = if fps > 0.0 { fps } else { DEF_FPS };

    let (argv, out_name) =
        gizza_ai_video_ken_burns_core::plan(direction, duration, zoom, width, height, fps, in_name)
            .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
