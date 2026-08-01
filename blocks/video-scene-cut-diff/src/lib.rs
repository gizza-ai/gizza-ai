//! gizza-ai/video-scene-cut-diff — detect the scene-cut timestamps in two edits
//! of the same footage and report which cuts were ADDED, REMOVED, or MOVED.
//!
//! Pipeline: resolve each of the two video sources (URL/ref, `AssetKind::Video`) →
//! run ffmpeg's `scene` detector over EACH video independently via the
//! gizza-ai/ffmpeg-runtime bridge (`select='gt(scene,threshold)',showinfo`, no
//! output file — the flagged frames' `pts_time` values land in the log) →
//! `core::parse_cuts` + `core::apply_min_scene` turn each log into a cut list →
//! `core::diff_cuts` matches the two lists within `tolerance` and classifies every
//! cut → flat JSON `Resp`.
//!
//! `Input::None` + a required two-item `videos` source_list (the loudness-compare /
//! video-audio-sync-offset-finder multi-input pattern). Each ffmpeg exec is
//! SINGLE-input and independent, so this sidesteps the un-buildable multi-input
//! ffmpeg case (video-concat) — only the pure `diff_cuts` step combines the two.
//!
//! Surfaces: chat + CLI (the ffmpeg bridge runs on both). No standalone page — the
//! generic page driver takes a single upload and one ffmpeg pass; this needs two of
//! each. Detection/diff is measurement-only: it never rewrites the videos.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, mime_to_ext, resolve_source, AssetKind, FfmpegReq, FfmpegResp,
};
use gizza_ai_video_scene_cut_diff_core as core;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Per-file input cap — matches the video tool family (compress or trim first).
const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

const DEFAULT_THRESHOLD: f64 = 0.3;
const DEFAULT_MIN_SCENE: f64 = 0.4;
const DEFAULT_TOLERANCE: f64 = 0.5;

#[derive(Deserialize, Debug)]
struct Args {
    videos: Vec<SourceFields>,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_min_scene")]
    min_scene: f64,
    #[serde(default = "default_tolerance")]
    tolerance: f64,
}
fn default_threshold() -> f64 {
    DEFAULT_THRESHOLD
}
fn default_min_scene() -> f64 {
    DEFAULT_MIN_SCENE
}
fn default_tolerance() -> f64 {
    DEFAULT_TOLERANCE
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("videos", 2)
                .required()
                .describe("Exactly two edits of the same footage to compare — a rough cut and a fine cut, a before/after re-edit, or two renders. Edit 1 (the first item) is the reference timeline; every added/removed/moved verdict is relative to it. Each item has exactly one of `url` or `ref`. Any container ffmpeg reads (MP4/MOV, MKV/WebM, AVI, …) with a video track; up to 32 MiB per file — compress or trim longer footage first."),
        )
        .param(
            Param::number("threshold")
                .default(DEFAULT_THRESHOLD)
                .min(0.0)
                .max(1.0)
                .describe("Scene-detection sensitivity, 0.0–1.0 (default 0.3). ffmpeg flags a frame as a scene cut when its visual difference from the previous frame exceeds this. Lower = more cuts (catches soft/graded transitions but also risks false positives on fast motion); higher = only hard cuts. 0.3–0.4 suits most footage."),
        )
        .param(
            Param::number("min_scene")
                .default(DEFAULT_MIN_SCENE)
                .min(0.0)
                .describe("Minimum seconds between detected cuts (default 0.4). A single hard cut can trip the detector on two adjacent frames; cuts closer than this to the previous one are merged so each transition counts once. 0 disables de-bouncing."),
        )
        .param(
            Param::number("tolerance")
                .default(DEFAULT_TOLERANCE)
                .min(0.0)
                .describe("Match window in seconds (default 0.5). A cut in edit 1 and a cut in edit 2 within this distance are treated as the SAME cut; if their times differ by more than ~0.04 s it is reported as MOVED, otherwise unchanged. Cuts with no partner within the window are ADDED (edit 2 only) or REMOVED (edit 1 only). Raise it if a global timeline shift is being reported as add+remove instead of moves."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[derive(Serialize)]
struct MovedResp {
    from: f64,
    to: f64,
    shift: f64,
}

#[derive(Serialize)]
struct SummaryResp {
    cuts_in_edit1: usize,
    cuts_in_edit2: usize,
    added: usize,
    removed: usize,
    moved: usize,
    unchanged: usize,
}

#[derive(Serialize)]
struct Resp {
    cuts_edit1: Vec<f64>,
    cuts_edit2: Vec<f64>,
    added: Vec<f64>,
    removed: Vec<f64>,
    moved: Vec<MovedResp>,
    unchanged: Vec<f64>,
    counts: SummaryResp,
    summary: String,
}

/// Build the flat JSON response from a core diff (shared by wasm + tests).
fn diff_json(diff: &core::CutDiff) -> Result<Vec<u8>, SkillError> {
    let resp = Resp {
        cuts_edit1: diff.cuts_a.clone(),
        cuts_edit2: diff.cuts_b.clone(),
        added: diff.added.clone(),
        removed: diff.removed.clone(),
        moved: diff
            .moved
            .iter()
            .map(|m| MovedResp {
                from: m.from,
                to: m.to,
                shift: m.shift,
            })
            .collect(),
        unchanged: diff.unchanged.clone(),
        counts: SummaryResp {
            cuts_in_edit1: diff.cuts_a.len(),
            cuts_in_edit2: diff.cuts_b.len(),
            added: diff.added.len(),
            removed: diff.removed.len(),
            moved: diff.moved.len(),
            unchanged: diff.unchanged.len(),
        },
        summary: diff.summary(),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize video-scene-cut-diff response: {e}")))
}

#[cfg(target_arch = "wasm32")]
struct VideoSceneCutDiff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-scene-cut-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect scene cuts in two edits of the same footage and list added, removed, or moved cuts",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Detect the scene-cut (shot-boundary) timestamps in TWO edits of the same footage and report which cuts were ADDED, REMOVED, or MOVED between them — an editorial diff of a rough vs fine cut, a before/after re-edit, or two renders. Provide videos as a list of exactly two items (edit 1, the reference timeline, first), each a url (HTTP/HTTPS) or a `ref` from a prior tool call; any container ffmpeg reads (MP4/MOV, MKV/WebM, AVI, …) with a video track, up to 32 MiB per file. ffmpeg's scene detector runs over each video independently: threshold (0.0–1.0, default 0.3) is the visual-difference sensitivity (lower = more cuts), and min_scene (seconds, default 0.4) merges cuts closer than that so each transition counts once. The two cut lists are then matched within tolerance seconds (default 0.5): a cut present in both within the window is unchanged, or MOVED if its time shifted by more than ~0.04 s; a cut with no partner is ADDED (edit 2 only) or REMOVED (edit 1 only). This tool DETECTS and DIFFS only — it never re-cuts or rewrites the videos. Returns flat JSON: cuts_edit1[]/cuts_edit2[] (all detected timestamps), added[]/removed[]/moved[]/unchanged[], a counts object, and a one-line summary.",
        parameters = schema_json()
    ),
)]
impl VideoSceneCutDiff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Run one ffmpeg-runtime exec and return the log (the scene detector writes no
/// output file). Maps a non-zero exit to `FfmpegExitNonZero` (last 200 log chars).
#[cfg(target_arch = "wasm32")]
fn detect_pass(argv: Vec<String>, in_name: &str, in_bytes: Vec<u8>) -> Result<String, SkillError> {
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(in_name.to_string(), in_bytes)],
        output: "detect.null".to_string(),
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let resp: FfmpegResp = serde_json::from_slice(&resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    if resp.exit_code != 0 {
        let tail: String = {
            let chars: Vec<char> = resp.log.chars().collect();
            let start = chars.len().saturating_sub(200);
            chars[start..].iter().collect()
        };
        return Err(SkillError::FfmpegExitNonZero {
            exit: resp.exit_code,
            snippet: tail,
        });
    }
    Ok(resp.log)
}

/// Detect + de-bounce one video's cut list, attributing errors to its label.
#[cfg(target_arch = "wasm32")]
fn cuts_for(
    source: SourceFields,
    label: &str,
    threshold: f64,
    min_scene: f64,
) -> Result<Vec<f64>, SkillError> {
    let (bytes, mime, _name) =
        resolve_source(source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime).unwrap_or("mp4");
    let in_name = format!("in.{ext}");
    let log = detect_pass(core::detect_argv(&in_name, threshold), &in_name, bytes)
        .map_err(|e| SkillError::InvalidArgs(format!("{label}: {e:?}")))?;
    let cuts = core::apply_min_scene(&core::parse_cuts(&log), min_scene);
    if cuts.len() > core::MAX_CUTS {
        return Err(SkillError::InvalidArgs(format!(
            "{label}: {} scene cuts detected — threshold {threshold} is too low for this footage; raise it",
            cuts.len()
        )));
    }
    Ok(cuts)
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-scene-cut-diff")?;
    if args.videos.len() != 2 {
        return Err(SkillError::InvalidArgs(format!(
            "video-scene-cut-diff needs exactly two videos (edit 1, the reference, first), got {}",
            args.videos.len()
        )));
    }
    if !args.threshold.is_finite() || !(0.0..=1.0).contains(&args.threshold) {
        return Err(SkillError::InvalidArgs(format!(
            "threshold must be between 0.0 and 1.0, got {}",
            args.threshold
        )));
    }
    if !args.min_scene.is_finite() || args.min_scene < 0.0 {
        return Err(SkillError::InvalidArgs(format!(
            "min_scene must be >= 0 seconds, got {}",
            args.min_scene
        )));
    }
    if !args.tolerance.is_finite() || args.tolerance < 0.0 {
        return Err(SkillError::InvalidArgs(format!(
            "tolerance must be >= 0 seconds, got {}",
            args.tolerance
        )));
    }

    let mut sources = args.videos.into_iter();
    let a = cuts_for(
        sources.next().expect("len checked"),
        "edit 1",
        args.threshold,
        args.min_scene,
    )?;
    let b = cuts_for(
        sources.next().expect("len checked"),
        "edit 2",
        args.threshold,
        args.min_scene,
    )?;

    let diff = core::diff_cuts(&a, &b, args.tolerance);
    diff_json(&diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "videos": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Exactly two edits of the same footage to compare — a rough cut and a fine cut, a before/after re-edit, or two renders. Edit 1 (the first item) is the reference timeline; every added/removed/moved verdict is relative to it. Each item has exactly one of `url` or `ref`. Any container ffmpeg reads (MP4/MOV, MKV/WebM, AVI, …) with a video track; up to 32 MiB per file — compress or trim longer footage first.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "threshold": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.3, "description": "Scene-detection sensitivity, 0.0–1.0 (default 0.3). ffmpeg flags a frame as a scene cut when its visual difference from the previous frame exceeds this. Lower = more cuts (catches soft/graded transitions but also risks false positives on fast motion); higher = only hard cuts. 0.3–0.4 suits most footage." },
                    "min_scene": { "type": "number", "minimum": 0, "default": 0.4, "description": "Minimum seconds between detected cuts (default 0.4). A single hard cut can trip the detector on two adjacent frames; cuts closer than this to the previous one are merged so each transition counts once. 0 disables de-bouncing." },
                    "tolerance": { "type": "number", "minimum": 0, "default": 0.5, "description": "Match window in seconds (default 0.5). A cut in edit 1 and a cut in edit 2 within this distance are treated as the SAME cut; if their times differ by more than ~0.04 s it is reported as MOVED, otherwise unchanged. Cuts with no partner within the window are ADDED (edit 2 only) or REMOVED (edit 1 only). Raise it if a global timeline shift is being reported as add+remove instead of moves." }
                },
                "required": ["videos"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn diff_json_shape_is_flat_and_complete() {
        let a = vec![1.0, 5.0, 10.0];
        let b = vec![1.0, 5.3, 12.0];
        let diff = core::diff_cuts(&a, &b, 0.5);
        let bytes = diff_json(&diff).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(v["cuts_edit1"], serde_json::json!([1.0, 5.0, 10.0]));
        assert_eq!(v["cuts_edit2"], serde_json::json!([1.0, 5.3, 12.0]));
        assert_eq!(v["added"], serde_json::json!([12.0]));
        assert_eq!(v["removed"], serde_json::json!([10.0]));
        assert_eq!(v["moved"][0]["from"], 5.0);
        assert_eq!(v["moved"][0]["to"], 5.3);
        assert_eq!(v["unchanged"], serde_json::json!([1.0]));
        assert_eq!(v["counts"]["cuts_in_edit1"], 3);
        assert_eq!(v["counts"]["added"], 1);
        assert_eq!(v["counts"]["removed"], 1);
        assert_eq!(v["counts"]["moved"], 1);
        assert_eq!(v["counts"]["unchanged"], 1);
        assert!(v["summary"].as_str().unwrap().contains("Edit 2 vs edit 1"));
    }
}
