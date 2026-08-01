//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-hum-remover/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `frequency`, then
//! `harmonics`, then `q`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)` and coerces numeric-looking field strings
//! to numbers first, so `harmonics`/`q` arrive as `f64`; `frequency` is the enum
//! string ("50"/"60", empty → 50).
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_audio_hum_remover_core::plan;
use wasm_bindgen::prelude::*;

/// `frequency` is the "50"/"60" enum (empty → 50). `harmonics` passes through as
/// an integer — 0 is a valid value (notch the fundamental only), so it is NOT
/// remapped. An empty `q` field arrives as `0.0`, which is below the accepted
/// range (1–100), so it maps to the `10` placeholder default. Returns
/// `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    frequency: &str,
    harmonics: f64,
    q: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let qv = if q == 0.0 { 10.0 } else { q };
    let (argv, out_name) =
        plan(in_name, frequency, harmonics as i64, qv).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
