//! Browser-facing wasm-bindgen wrapper for /tools/video-autocrop-bars/.
//!
//! The page is TWO-pass (like the chat/CLI block), driven by `page/custom.js`:
//!   1. `detect_argv(threshold, round, in_name)` → cropdetect plan (no output
//!      file; the page runs it and keeps the ffmpeg LOG);
//!   2. `crop_plan(log, in_name)` → either `{ no_bars: true, in_w, in_h }` or
//!      the crop pass `{ argv, out_name, w, h, x, y, in_w, in_h }`.
//! All parsing/decision logic is shared with the chat block via `core`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_autocrop_bars_core as core;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Pass-1 plan. `threshold` empty/NaN falls back to the default (24); `round`
/// empty falls back to "2". Field order matches page/meta.toml.
#[wasm_bindgen]
pub fn detect_argv(threshold: f64, round: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let threshold = if threshold.is_nan() { core::DEFAULT_THRESHOLD } else { threshold };
    let round = if round.trim().is_empty() { core::DEFAULT_ROUND } else { round };
    let (t, r) = core::validate(threshold, round).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan {
        argv: core::detect_argv(in_name, t, r),
        out_name: "detect.null".into(),
    })
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Serialize)]
struct CropPlan {
    no_bars: bool,
    in_w: u32,
    in_h: u32,
    w: u32,
    h: u32,
    x: u32,
    y: u32,
    argv: Vec<String>,
    out_name: String,
}

/// Pass-2 plan from the detect pass's ffmpeg log. Errors (unreadable log,
/// whole-frame-black threshold) come back as strings the page shows verbatim.
#[wasm_bindgen]
pub fn crop_plan(log: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let plan = match core::decide(log).map_err(|e| JsValue::from_str(&e))? {
        core::Decision::NoBars { in_w, in_h } => CropPlan {
            no_bars: true,
            in_w,
            in_h,
            w: in_w,
            h: in_h,
            x: 0,
            y: 0,
            argv: vec![],
            out_name: String::new(),
        },
        core::Decision::Crop { w, h, x, y, in_w, in_h } => {
            let (argv, out_name) = core::crop_argv(in_name, w, h, x, y);
            CropPlan { no_bars: false, in_w, in_h, w, h, x, y, argv, out_name }
        }
    };
    serde_wasm_bindgen::to_value(&plan).map_err(|e| JsValue::from_str(&e.to_string()))
}
