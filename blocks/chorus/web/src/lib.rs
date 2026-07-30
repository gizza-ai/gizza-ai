//! Browser-facing wasm-bindgen wrapper for /tools/chorus/ (ffmpeg page).
//! Page field order (meta.toml) MUST match this param order, followed by the
//! uploaded file's `in_name`. The runtime executes the returned argv via ffmpeg.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_chorus_core::plan_chorus;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    voices: f64,
    delay_ms: f64,
    depth_ms: f64,
    speed_hz: f64,
    decay: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // The page runtime hands numeric fields over as JS Numbers; keep this an f64
    // (not i64) so wasm-bindgen doesn't demand a BigInt, then round to a voice count.
    let voices = voices.round() as i64;
    let (argv, out_name) =
        plan_chorus(in_name, voices, delay_ms, depth_ms, speed_hz, decay, format)
            .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
