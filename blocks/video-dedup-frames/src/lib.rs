//! gizza-ai/video-dedup-frames — fetch a video URL or attachment ref and drop
//! consecutive duplicate frames with ffmpeg's `mpdecimate` filter (screen
//! recordings, slideshow exports, animation renders). `sensitivity` scales the
//! filter's hi/lo thresholds, `frac` its changed-block fraction, `max_fps` caps
//! the rate before the decimate, `timing` decides whether the freed time is kept
//! (VFR), re-held on an even grid (CFR) or closed up (a shorter clip), and
//! `format` picks the output container.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); source-resolution, ffmpeg dispatch and
//! envelope-building are delegated to `block_utils`, while validation and the
//! pure argv builder live in `core`.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_dedup_frames_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Duplicate-detection sensitivity 1–100; omitted → core default (50).
    #[serde(default)]
    sensitivity: Option<f64>,
    #[serde(default)]
    timing: Option<String>,
    /// Frame-rate cap applied before the decimate; omitted → source rate.
    #[serde(default)]
    max_fps: Option<f64>,
    #[serde(default)]
    format: Option<String>,
    /// mpdecimate `frac`; omitted → core default (0.33).
    #[serde(default)]
    frac: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("sensitivity")
                .min(1.0)
                .max(100.0)
                .describe("How eagerly frames count as duplicates, 1–100 (default 50). It scales ffmpeg mpdecimate's thresholds linearly around their native values at 50 (hi=768, lo=320): 25 → hi=384/lo=160 (only near-identical frames go), 100 → hi=1536/lo=640 (frames that merely look the same — cursor blinks, dithering, compression shimmer — go too). Clamped to 1–100."),
        )
        .param(
            Param::enumv("timing", ["keep", "constant", "compact"])
                .default("keep")
                .describe("What happens to the time the dropped frames occupied: keep (default) leaves every remaining frame at its original timestamp, so the clip plays with identical timing as a variable-frame-rate file; constant re-holds the kept frames on an even grid (constant frame rate, editor-friendly, still shrinks because near-duplicates become exact repeats); compact closes the gaps so the clip shortens to just the frames that changed — audio is dropped in that mode because it cannot follow the re-timing."),
        )
        .param(
            Param::number("max_fps")
                .min(1.0)
                .max(240.0)
                .describe("Optional frame-rate cap applied BEFORE the duplicate scan, e.g. 30 to halve a 60 fps screen capture (1–240; omit to keep the source rate). Capping first is safe: if the source is slower than the cap, the frames it inserts are removed again by the duplicate scan."),
        )
        .param(
            Param::enumv("format", ["auto", "mp4", "webm"])
                .default("auto")
                .describe("Output container: auto (default) keeps the input container when it can hold H.264/AAC (mp4/mov/m4v/mkv) and converts anything else (e.g. webm) to MP4; mp4 forces H.264/AAC; webm forces VP9/Opus. Filtering always forces a re-encode, so the output is never byte-identical to the source."),
        )
        .param(
            Param::number("frac")
                .min(0.01)
                .max(1.0)
                .describe("Advanced: mpdecimate's frac — the fraction of a frame's 8x8 blocks that must change for the frame to be kept (0.01–1, default 0.33). Lower keeps more frames (a small changing region like a cursor or subtitle counts as motion); higher drops more (only a big change keeps a frame)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}

#[cfg(target_arch = "wasm32")]
struct VideoDedupFrames;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-dedup-frames",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Drop consecutive duplicate frames from a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Drop consecutive duplicate frames from a video (screen recordings, slideshow exports, animation renders) with ffmpeg's mpdecimate filter, shrinking the file and cleaning up the timing. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call), plus sensitivity (1-100, how eagerly frames count as duplicates; default 50 = ffmpeg's native thresholds), timing (keep = same timing, variable frame rate, default; constant = even frame rate for editors; compact = close the gaps so the clip gets shorter, audio dropped), max_fps (optional cap applied before the scan, e.g. 30 to halve a 60 fps capture), format (auto|mp4|webm) and frac (advanced mpdecimate changed-block fraction, default 0.33). The picture is re-encoded (H.264 crf 20, or VP9 for webm); audio is stream-copied when the container allows it. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoDedupFrames {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args. Non-finite numbers are rejected by core's `plan`; unset /
    //    out-of-range values resolve to the documented defaults there too. The
    //    page's "unset" sentinel is 0.0, so None → 0.0 hits the same path.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-dedup-frames")?;
    let sensitivity = args.sensitivity.unwrap_or(0.0);
    let timing = args.timing.as_deref().unwrap_or("keep");
    let max_fps = args.max_fps.unwrap_or(0.0);
    let format = args.format.as_deref().unwrap_or("auto");
    let frac = args.frac.unwrap_or(0.0);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates every param).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(sensitivity, timing, max_fps, format, frac, &ffmpeg_in)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-dedup", out_ext);
    let for_llm = format!(
        "de-duplicated {in_filename} (sensitivity {sensitivity}, timing {timing}) ({output_size} bytes {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + the five params), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":         { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "sensitivity": { "type": "number", "minimum": 1, "maximum": 100, "description": "How eagerly frames count as duplicates, 1–100 (default 50). It scales ffmpeg mpdecimate's thresholds linearly around their native values at 50 (hi=768, lo=320): 25 → hi=384/lo=160 (only near-identical frames go), 100 → hi=1536/lo=640 (frames that merely look the same — cursor blinks, dithering, compression shimmer — go too). Clamped to 1–100." },
                    "timing":      { "type": "string", "enum": ["keep", "constant", "compact"], "default": "keep", "description": "What happens to the time the dropped frames occupied: keep (default) leaves every remaining frame at its original timestamp, so the clip plays with identical timing as a variable-frame-rate file; constant re-holds the kept frames on an even grid (constant frame rate, editor-friendly, still shrinks because near-duplicates become exact repeats); compact closes the gaps so the clip shortens to just the frames that changed — audio is dropped in that mode because it cannot follow the re-timing." },
                    "max_fps":     { "type": "number", "minimum": 1, "maximum": 240, "description": "Optional frame-rate cap applied BEFORE the duplicate scan, e.g. 30 to halve a 60 fps screen capture (1–240; omit to keep the source rate). Capping first is safe: if the source is slower than the cap, the frames it inserts are removed again by the duplicate scan." },
                    "format":      { "type": "string", "enum": ["auto", "mp4", "webm"], "default": "auto", "description": "Output container: auto (default) keeps the input container when it can hold H.264/AAC (mp4/mov/m4v/mkv) and converts anything else (e.g. webm) to MP4; mp4 forces H.264/AAC; webm forces VP9/Opus. Filtering always forces a re-encode, so the output is never byte-identical to the source." },
                    "frac":        { "type": "number", "minimum": 0.01, "maximum": 1, "description": "Advanced: mpdecimate's frac — the fraction of a frame's 8x8 blocks that must change for the frame to be kept (0.01–1, default 0.33). Lower keeps more frames (a small changing region like a cursor or subtitle counts as motion); higher drops more (only a big change keeps a frame)." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_dedup_suffix() {
        assert_eq!(
            filename_with_suffix("screen-recording.mov", "-dedup", "mov"),
            "screen-recording-dedup.mov"
        );
        // A forced/auto container switch changes the extension too.
        assert_eq!(
            filename_with_suffix("capture.webm", "-dedup", "mp4"),
            "capture-dedup.mp4"
        );
    }
}
