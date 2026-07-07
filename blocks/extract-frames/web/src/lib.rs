//! Browser-facing wasm-bindgen wrapper for /tools/extract-frames/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (page/meta.toml) MUST match this param order: `mode`,
//! `value`, `columns`, `rows`, `width`, `background`, `format`, then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`. Numeric
//! fields (`value`/`columns`/`rows`/`width`) arrive as JS numbers → `f64`;
//! `mode`/`background`/`format` are strings. Empty numeric fields arrive as
//! `0.0`, which the core rejects with a guiding message.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `mode` ∈ interval|fps|scene; `value` is mode-dependent (seconds / fps /
/// scene threshold 0–1); `columns`×`rows` is the grid; `width` is the
/// per-thumbnail width in px; `background` is the grid-gap color; `format` ∈
/// png|jpg. Returns `{ argv, out_name }` or throws the validation error string.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    mode: &str,
    value: f64,
    columns: f64,
    rows: f64,
    width: f64,
    background: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // f64 → u32 saturates (Rust `as`), so negatives/NaN become 0 and absurd
    // values saturate to u32::MAX — the core then rejects both out of range.
    let (argv, out_name) = gizza_ai_extract_frames_core::plan(
        in_name,
        mode,
        value,
        columns as u32,
        rows as u32,
        width as u32,
        background,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
