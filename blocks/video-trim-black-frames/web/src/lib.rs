//! Browser-facing wasm-bindgen wrapper for /tools/video-trim-black-frames/.
//!
//! The page is TWO-pass (like the chat/CLI block), driven by `page/custom.js`:
//!   1. `detect_argv(pixel_threshold, black_ratio, min_duration, ends, in_name)`
//!      → blackdetect plan (no output file; the page runs it and keeps the LOG);
//!   2. `trim_plan(log, ends, in_name)` → either `{ no_edges: true, duration }`
//!      or the trim pass `{ argv, out_name, start, end, duration, removed_start,
//!      removed_end }`.
//! All parsing/decision logic is shared with the chat block via `core`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_trim_black_frames_core as core;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Pass-1 plan. Empty/NaN numeric fields fall back to their defaults; an empty
/// `ends` falls back to "both". Field order matches page/meta.toml.
#[wasm_bindgen]
pub fn detect_argv(
    pixel_threshold: f64,
    black_ratio: f64,
    min_duration: f64,
    ends: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let pixel_threshold =
        if pixel_threshold.is_nan() { core::DEFAULT_PIXEL_THRESHOLD } else { pixel_threshold };
    let black_ratio = if black_ratio.is_nan() { core::DEFAULT_BLACK_RATIO } else { black_ratio };
    let min_duration = if min_duration.is_nan() { core::DEFAULT_MIN_DURATION } else { min_duration };
    let ends = if ends.trim().is_empty() { core::DEFAULT_ENDS } else { ends };
    let (pix, ratio, dur, _ends) = core::validate(pixel_threshold, black_ratio, min_duration, ends)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan {
        argv: core::detect_argv(in_name, pix, ratio, dur),
        out_name: "detect.null".into(),
    })
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Serialize)]
struct TrimPlan {
    no_edges: bool,
    start: f64,
    end: f64,
    duration: f64,
    removed_start: f64,
    removed_end: f64,
    argv: Vec<String>,
    out_name: String,
}

/// Pass-2 plan from the detect pass's ffmpeg log. Errors (unreadable log,
/// whole-clip-black) come back as strings the page shows verbatim.
#[wasm_bindgen]
pub fn trim_plan(log: &str, ends: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let ends = if ends.trim().is_empty() { core::DEFAULT_ENDS } else { ends };
    let ends = core::parse_ends(ends).map_err(|e| JsValue::from_str(&e))?;
    let plan = match core::decide(log, ends).map_err(|e| JsValue::from_str(&e))? {
        core::TrimDecision::NoEdges { duration } => TrimPlan {
            no_edges: true,
            start: 0.0,
            end: duration,
            duration,
            removed_start: 0.0,
            removed_end: 0.0,
            argv: vec![],
            out_name: String::new(),
        },
        core::TrimDecision::Trim { start, end, duration } => {
            let (argv, out_name) = core::trim_argv(in_name, start, end);
            let (removed_start, removed_end) = core::removed(start, end, duration);
            TrimPlan {
                no_edges: false,
                start,
                end,
                duration,
                removed_start,
                removed_end,
                argv,
                out_name,
            }
        }
    };
    serde_wasm_bindgen::to_value(&plan).map_err(|e| JsValue::from_str(&e.to_string()))
}
