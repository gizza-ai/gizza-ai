//! Browser-facing wasm-bindgen wrapper for /tools/video-aspect-pad/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }. Field order in page/meta.toml MUST equal this param
//! order: aspect, width, color, blur, quality. width 0/blank means
//! "platform-standard canvas for the aspect"; color blank means black; blur
//! arrives as the checkbox string "true"/"false" (page fields are strings);
//! quality blank means medium.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    aspect: &str,
    width: f64,
    color: &str,
    blur: &str,
    quality: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let w = if width > 0.0 { Some(width as u32) } else { None };
    let blur = matches!(blur.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    let (argv, out_name) =
        gizza_ai_video_aspect_pad_core::plan(in_name, aspect, w, color, blur, quality)
            .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
