//! Browser-facing wasm-bindgen wrapper for /tools/video-scene-split/.
//!
//! The page is MULTI-pass (like the chat/CLI block), driven by `page/custom.js`:
//!   1. `detect_argv(threshold, in_name)` → the scene-detector plan (no output
//!      file; the page runs it and keeps the ffmpeg LOG);
//!   2. `scene_plan(log, threshold, min_scene, mode, crf, preset, keep_audio,
//!      in_name, filename)` → either `{ single: true, … }` (no cut found) or the
//!      full scene table, each entry carrying its own extract `argv`/`out_name`
//!      plus the download name, and the `csv` timing export.
//! All parsing/window/argv logic is shared with the chat block via `core`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_scene_split_core as core;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Pass-1 plan. An empty/NaN `threshold` falls back to the default.
#[wasm_bindgen]
pub fn detect_argv(threshold: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let threshold = if threshold.is_nan() {
        core::DEFAULT_THRESHOLD
    } else {
        threshold
    };
    if !(0.0..=1.0).contains(&threshold) {
        return Err(JsValue::from_str(&format!(
            "threshold must be between 0.0 and 1.0 (got {threshold})"
        )));
    }
    serde_wasm_bindgen::to_value(&ArgvPlan {
        argv: core::detect_argv(in_name, threshold),
        out_name: "detect.null".into(),
    })
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Serialize)]
struct ScenePlan {
    index: usize,
    start: f64,
    end: f64,
    duration: f64,
    argv: Vec<String>,
    out_name: String,
    download_name: String,
}

#[derive(Serialize)]
struct SplitPlan {
    /// True when the detector found no cut — the page shows a friendly note
    /// instead of re-encoding one clip identical to the input.
    single: bool,
    duration: f64,
    threshold: f64,
    min_scene: f64,
    count: usize,
    csv: String,
    scenes: Vec<ScenePlan>,
}

/// Pass-2 plan from the detection pass's ffmpeg log. Errors (unreadable
/// duration, bad knob, over the clip cap) come back as strings the page shows
/// verbatim. `filename` is the uploaded file's name — the clip download names
/// are derived from it.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn scene_plan(
    log: &str,
    threshold: f64,
    min_scene: f64,
    mode: &str,
    crf: f64,
    preset: &str,
    keep_audio: &str,
    in_name: &str,
    filename: &str,
) -> Result<JsValue, JsValue> {
    let params = core::validate(core::Params {
        threshold: if threshold.is_nan() { core::DEFAULT_THRESHOLD } else { threshold },
        min_scene: if min_scene.is_nan() { core::DEFAULT_MIN_SCENE } else { min_scene },
        mode: if mode.trim().is_empty() { core::DEFAULT_MODE.into() } else { mode.into() },
        crf: if crf.is_nan() { core::DEFAULT_CRF } else { crf as i64 },
        preset: if preset.trim().is_empty() { core::DEFAULT_PRESET.into() } else { preset.into() },
        // The page marshals checkboxes as "true"/"false"; parse positive-truthy.
        keep_audio: matches!(keep_audio, "true" | "1" | "on" | "yes"),
    })
    .map_err(|e| JsValue::from_str(&e))?;

    let duration = core::parse_duration(log).unwrap_or(0.0);
    let cuts = core::apply_min_scene(&core::parse_cuts(log), params.min_scene);
    let scenes = core::build_scenes(&cuts, duration, params.min_scene)
        .map_err(|e| JsValue::from_str(&e))?;

    let in_ext = in_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let ext = core::clip_ext(in_ext, &params.mode);
    let stem = core::safe_stem(filename);
    let last = scenes.len() - 1;
    let plan = SplitPlan {
        single: scenes.len() < 2,
        duration,
        threshold: params.threshold,
        min_scene: params.min_scene,
        count: scenes.len(),
        csv: core::scenes_csv(&scenes, &stem, ext),
        scenes: scenes
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let (argv, out_name) = core::clip_argv(in_name, s, i == last, &params, ext);
                ScenePlan {
                    index: s.index,
                    start: s.start,
                    end: s.end,
                    duration: s.duration(),
                    argv,
                    out_name,
                    download_name: s.entry_name(&stem, ext),
                }
            })
            .collect(),
    };
    serde_wasm_bindgen::to_value(&plan).map_err(|e| JsValue::from_str(&e.to_string()))
}
