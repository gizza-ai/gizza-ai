//! Browser-facing wasm-bindgen wrapper for /tools/audio-effects-rack/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `reverb`, `echo`,
//! `chorus`, `tremolo`, `compression`, then `format`, then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_effects_rack_core::plan_effects;
use gizza_ai_block_utils::ArgvPlan;

/// `reverb` is `none|room|hall|plate`, `chorus` `none|light|deep`,
/// `compression` `none|light|medium|heavy` (empty selects default to their
/// first `none` value). `echo` is a delay in ms (0–1000; 0 = off) and `tremolo`
/// a rate in Hz (0 or 0.1–20; 0 = off) — empty fields arrive as 0. `format` is
/// `mp3|wav|ogg|flac|m4a` (empty defaults to mp3). Every stage off throws the
/// guiding no-op error. Returns `{ argv, out_name }` or throws an error string.
#[wasm_bindgen]
pub fn build_argv(
    reverb: &str,
    echo: f64,
    chorus: &str,
    tremolo: f64,
    compression: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_effects(in_name, reverb, echo, chorus, tremolo, compression, format)
            .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
