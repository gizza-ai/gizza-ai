//! gizza-ai/video-scene-split — fetch a video (url⊕ref), detect its shot
//! boundaries, and cut it into one clip per scene, returned as a ZIP.
//!
//! Multi-pass flow (same detect-then-act shape as `video-autocrop-bars`, but the
//! second phase runs once per scene): pass 1 runs ffmpeg's `scene` detector with
//! `showinfo` and no output file, and the shared `core` parses the flagged
//! `pts_time` values plus the input `Duration:` banner out of the log; the cuts
//! are de-bounced by `min_scene` and turned into `[start, end)` windows; then one
//! extract pass per window produces the clip (H.264/AAC re-encode by default, or
//! a near-instant stream copy). The clips plus a `scenes.csv` timing table are
//! bundled into a single ZIP envelope so one tool call returns everything.
//!
//! The chat schema is derived from `descriptor()` (single source, shared with the
//! CLI + page); every pure part (argv, log parsing, scene windows, CSV) lives in
//! `core` and is unit-tested there. The standalone page mirrors this in
//! `page/custom.js`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_runtime, resolve_source, FfmpegReq, FfmpegResp};
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_video_scene_split_core as core;
use serde::Deserialize;
use std::io::{Cursor, Write};
use wafer_sdk::*;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    threshold: Option<f64>,
    #[serde(default)]
    min_scene: Option<f64>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    crf: Option<i64>,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    keep_audio: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("threshold")
                .min(0.0)
                .max(1.0)
                .default(core::DEFAULT_THRESHOLD)
                .describe(
                    "Scene-detection sensitivity, 0.0-1.0 (default 0.3). ffmpeg flags a frame as a \
                     shot boundary when its visual difference from the previous frame exceeds this. \
                     Lower (0.15-0.25) catches soft or graded transitions but also fast motion; \
                     higher (0.4-0.6) keeps only hard cuts. 0.3-0.4 suits most footage.",
                ),
        )
        .param(
            Param::number("min_scene")
                .min(0.0)
                .default(core::DEFAULT_MIN_SCENE)
                .describe(
                    "Shortest scene to emit, in seconds (default 0.6). Boundaries closer together \
                     than this are merged, so one hard cut spread over two frames produces one \
                     clip; a trailing scene shorter than this is folded into the one before it. \
                     0 disables merging.",
                ),
        )
        .param(
            Param::enumv("mode", core::MODES)
                .default(core::DEFAULT_MODE)
                .describe(
                    "How each clip is cut. 'reencode' (default) re-encodes to H.264/AAC MP4 so \
                     every clip starts exactly on its detected boundary. 'copy' remuxes the \
                     original packets — near-instant and lossless, but each clip snaps back to the \
                     previous keyframe, so starts can be off by up to a GOP, and the source \
                     container is kept.",
                ),
        )
        .param(
            Param::integer("crf")
                .min(0.0)
                .max(51.0)
                .default(core::DEFAULT_CRF)
                .describe(
                    "x264 quality for mode=reencode: 0 (lossless, huge) to 51 (worst), default 22. \
                     Lower is better quality and a bigger file; 18-23 is the usual range. Ignored \
                     when mode=copy.",
                ),
        )
        .param(
            Param::enumv("preset", core::PRESETS)
                .default(core::DEFAULT_PRESET)
                .describe(
                    "x264 speed/compression preset for mode=reencode (default veryfast). Slower \
                     presets (medium, slow, veryslow) give smaller files for the same crf; faster \
                     ones (ultrafast, superfast) finish sooner. Ignored when mode=copy.",
                ),
        )
        .param(
            Param::boolean("keep_audio")
                .default(true)
                .describe(
                    "Keep the audio track in each clip (default true). Set false to write \
                     video-only clips (ffmpeg -an) — smaller files for B-roll or silent inserts.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-scene-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect scene changes in a video and split it into one clip per scene, bundled as a ZIP",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Detect the scene (shot) changes in a video and split it into one clip per scene. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call); any container ffmpeg reads (MP4/MOV, MKV/WebM, AVI, ...) with a video track, up to 25 MiB. Pass 1 runs ffmpeg's scene detector: threshold (0.0-1.0, default 0.3) is the visual-difference sensitivity (lower = more cuts) and min_scene (seconds, default 0.6) merges boundaries closer than that so each transition yields one clip. Each detected scene is then extracted: mode=reencode (default) writes frame-accurate H.264/AAC MP4 clips at crf (0-51, default 22) and preset (default veryfast); mode=copy remuxes losslessly but snaps each start to the previous keyframe and keeps the source container. keep_audio=false drops audio. Returns ONE application/zip file containing <name>-Scene-001.<ext>, ... plus scenes.csv (scene, start_seconds, end_seconds, duration_seconds, filename). Capped at 200 clips. If no scene change is found the tool says so instead of returning a single clip identical to the input.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Run one ffmpeg-runtime exec, returning the full response so the detect pass
/// can read the log and each extract pass the output bytes.
#[cfg(target_arch = "wasm32")]
fn run_pass(
    argv: Vec<String>,
    in_name: &str,
    in_bytes: Vec<u8>,
    out_name: &str,
) -> Result<FfmpegResp, SkillError> {
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(in_name.to_string(), in_bytes)],
        output: out_name.to_string(),
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let resp: FfmpegResp = serde_json::from_slice(&resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    if resp.exit_code != 0 {
        return Err(SkillError::FfmpegExitNonZero {
            exit: resp.exit_code,
            snippet: resp.log.chars().take(200).collect(),
        });
    }
    Ok(resp)
}

/// Bundle the per-scene clips + the CSV timing table into one ZIP. Clips are
/// STORED (already-compressed video gains nothing from deflate and the pass is
/// not free); the CSV is deflated.
fn build_zip(clips: &[(String, Vec<u8>)], csv: &str) -> Result<Vec<u8>, String> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("scenes.csv", deflated)
        .map_err(|e| format!("zip error: {e}"))?;
    zip.write_all(csv.as_bytes())
        .map_err(|e| format!("zip write error: {e}"))?;
    for (name, bytes) in clips {
        zip.start_file(name, stored)
            .map_err(|e| format!("zip error: {e}"))?;
        zip.write_all(bytes)
            .map_err(|e| format!("zip write error: {e}"))?;
    }
    Ok(zip
        .finish()
        .map_err(|e| format!("zip finalize error: {e}"))?
        .into_inner())
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::{mime_to_ext, AssetKind};

    // 1. Parse + validate args before any fetch, so bad args fail fast.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-scene-split")?;
    let params = core::validate(core::Params {
        threshold: args.threshold.unwrap_or(core::DEFAULT_THRESHOLD),
        min_scene: args.min_scene.unwrap_or(core::DEFAULT_MIN_SCENE),
        mode: args.mode.unwrap_or_else(|| core::DEFAULT_MODE.to_string()),
        crf: args.crf.unwrap_or(core::DEFAULT_CRF),
        preset: args.preset.unwrap_or_else(|| core::DEFAULT_PRESET.to_string()),
        keep_audio: args.keep_audio.unwrap_or(true),
    })
    .map_err(|e| SkillError::InvalidArgs(format!("invalid video-scene-split args: {e}")))?;

    // 2. Resolve source.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let stem = core::safe_stem(&in_filename);
    let clip_ext = core::clip_ext(in_ext, &params.mode);

    // 3. Pass 1 — detect the shot boundaries (no output file; the log is the result).
    let detect = run_pass(
        core::detect_argv(&ffmpeg_in, params.threshold),
        &ffmpeg_in,
        input_bytes.clone(),
        "detect.null",
    )?;
    let duration = core::parse_duration(&detect.log).unwrap_or(0.0);
    let cuts = core::apply_min_scene(&core::parse_cuts(&detect.log), params.min_scene);
    let scenes =
        core::build_scenes(&cuts, duration, params.min_scene).map_err(SkillError::InvalidArgs)?;
    if scenes.len() < 2 {
        return Err(SkillError::InvalidArgs(format!(
            "no scene changes detected in {in_filename} at threshold {} — lower the threshold \
             (e.g. 0.15) for soft or graded transitions, or reduce min_scene ({}s) if the cuts \
             are closer together than that",
            core::fmt_num(params.threshold),
            core::fmt_num(params.min_scene),
        )));
    }

    // 4. One extract pass per scene.
    let last = scenes.len() - 1;
    let mut clips: Vec<(String, Vec<u8>)> = Vec::with_capacity(scenes.len());
    let mut total = 0usize;
    for (i, scene) in scenes.iter().enumerate() {
        let (argv, out_name) = core::clip_argv(&ffmpeg_in, scene, i == last, &params, clip_ext);
        let resp = run_pass(argv, &ffmpeg_in, input_bytes.clone(), &out_name)?;
        total += resp.output.len();
        if total > MAX_OUTPUT_BYTES {
            return Err(SkillError::InvalidArgs(format!(
                "the {} clips exceed the {MAX_OUTPUT_BYTES}-byte output limit — raise crf, use a \
                 faster preset, or split a shorter section",
                scenes.len()
            )));
        }
        clips.push((scene.entry_name(&stem, clip_ext), resp.output));
    }

    // 5. Bundle the clips + the timing table into one ZIP envelope.
    let csv = core::scenes_csv(&scenes, &stem, clip_ext);
    let zip = build_zip(&clips, &csv).map_err(SkillError::InvalidArgs)?;
    let filename = replace_extension(&in_filename, "zip");
    let for_llm = format!(
        "{} Clips: {} — bundled with scenes.csv into {filename} ({}-byte ZIP).",
        core::summary(&scenes, &params, &in_filename),
        clips
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        zip.len()
    );
    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url: format!("data:application/zip;base64,{}", B64.encode(&zip)),
            mime: "application/zip".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift. Regenerate this literal (never
    /// hand-patch it) whenever `descriptor()` changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold":  { "type": "number", "minimum": 0, "maximum": 1, "default": 0.3, "description": "Scene-detection sensitivity, 0.0-1.0 (default 0.3). ffmpeg flags a frame as a shot boundary when its visual difference from the previous frame exceeds this. Lower (0.15-0.25) catches soft or graded transitions but also fast motion; higher (0.4-0.6) keeps only hard cuts. 0.3-0.4 suits most footage." },
                    "min_scene":  { "type": "number", "minimum": 0, "default": 0.6, "description": "Shortest scene to emit, in seconds (default 0.6). Boundaries closer together than this are merged, so one hard cut spread over two frames produces one clip; a trailing scene shorter than this is folded into the one before it. 0 disables merging." },
                    "mode":       { "type": "string", "enum": ["reencode", "copy"], "default": "reencode", "description": "How each clip is cut. 'reencode' (default) re-encodes to H.264/AAC MP4 so every clip starts exactly on its detected boundary. 'copy' remuxes the original packets — near-instant and lossless, but each clip snaps back to the previous keyframe, so starts can be off by up to a GOP, and the source container is kept." },
                    "crf":        { "type": "integer", "minimum": 0, "maximum": 51, "default": 22, "description": "x264 quality for mode=reencode: 0 (lossless, huge) to 51 (worst), default 22. Lower is better quality and a bigger file; 18-23 is the usual range. Ignored when mode=copy." },
                    "preset":     { "type": "string", "enum": ["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "veryslow"], "default": "veryfast", "description": "x264 speed/compression preset for mode=reencode (default veryfast). Slower presets (medium, slow, veryslow) give smaller files for the same crf; faster ones (ultrafast, superfast) finish sooner. Ignored when mode=copy." },
                    "keep_audio": { "type": "boolean", "default": true, "description": "Keep the audio track in each clip (default true). Set false to write video-only clips (ffmpeg -an) — smaller files for B-roll or silent inserts." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    /// The ZIP always leads with scenes.csv and carries one entry per clip,
    /// under the user-facing `<stem>-Scene-NNN.<ext>` names.
    #[test]
    fn zip_bundles_the_csv_and_every_clip() {
        let scenes = core::build_scenes(&[1.0, 2.0], 3.0, 0.6).unwrap();
        let csv = core::scenes_csv(&scenes, "clip", "mp4");
        let clips: Vec<(String, Vec<u8>)> = scenes
            .iter()
            .map(|s| (s.entry_name("clip", "mp4"), vec![0u8; 16]))
            .collect();
        let bytes = build_zip(&clips, &csv).unwrap();
        let mut ar = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..ar.len())
            .map(|i| ar.by_index(i).unwrap().name().to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "scenes.csv",
                "clip-Scene-001.mp4",
                "clip-Scene-002.mp4",
                "clip-Scene-003.mp4"
            ]
        );
    }

    /// Error path: the output filename is always the source name with a .zip
    /// extension, whatever the input container was.
    #[test]
    fn output_filename_is_the_source_name_as_zip() {
        assert_eq!(replace_extension("holiday.mov", "zip"), "holiday.zip");
    }
}
