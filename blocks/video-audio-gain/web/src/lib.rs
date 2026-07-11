//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-gain/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `amount`, then
//! `unit`, then `limiter`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_audio_gain_core::plan;
use wasm_bindgen::prelude::*;

/// An empty amount field arrives as `0.0`, which is the "wouldn't change
/// anything" error for db — treat it as the +6 dB placeholder instead so the
/// leave-it-blank flow boosts, matching the page placeholder. `unit` is
/// `db|factor` (empty → db); `limiter` is the checkbox value string (positive
/// truthy). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    amount: f64,
    unit: &str,
    limiter: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let amt = if amount == 0.0 && matches!(unit, "" | "db") {
        6.0
    } else {
        amount
    };
    let limit_on = matches!(limiter, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan(in_name, amt, unit, limit_on).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
