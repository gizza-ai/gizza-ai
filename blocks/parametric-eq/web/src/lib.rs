//! Browser-facing wasm-bindgen wrapper for /tools/parametric-eq/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: the three bands'
//! `freq`, `gain`, `q` triplets, then `format`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.
//!
//! Empty numeric fields arrive as 0. A band with gain 0 is left off entirely,
//! so its freq/Q don't matter; but a band the user turned on (non-zero gain)
//! with an empty freq/Q field would otherwise send freq 0 / Q 0 and be
//! rejected — so empty freq/Q are backfilled to the same defaults the chat/CLI
//! surface uses before planning.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_parametric_eq_core::{plan_eq, Band, DEFAULT_FREQS, DEFAULT_Q};
use wasm_bindgen::prelude::*;

fn band(freq: f64, gain: f64, q: f64, idx: usize) -> Band {
    Band::new(
        if freq > 0.0 { freq } else { DEFAULT_FREQS[idx] },
        gain,
        if q > 0.0 { q } else { DEFAULT_Q },
    )
}

/// Three parametric bands (`freqN`/`gainN`/`qN`, gains in dB; a band with gain 0
/// is skipped, all three at 0 throws the guiding no-op error) plus `format`
/// (`mp3|wav|ogg|flac|m4a`, empty defaults to mp3). Returns `{ argv, out_name }`
/// or throws an error string.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    band1_freq: f64,
    band1_gain: f64,
    band1_q: f64,
    band2_freq: f64,
    band2_gain: f64,
    band2_q: f64,
    band3_freq: f64,
    band3_gain: f64,
    band3_q: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let bands = [
        band(band1_freq, band1_gain, band1_q, 0),
        band(band2_freq, band2_gain, band2_q, 1),
        band(band3_freq, band3_gain, band3_q, 2),
    ];
    let (argv, out_name) = plan_eq(in_name, &bands, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
