//! gizza-ai/video-deinterlace — fetch a video URL or attachment ref and remove
//! interlacing combing with ffmpeg's motion-adaptive `bwdif`/`yadif` filters,
//! writing clean progressive frames. `mode` chooses between keeping the frame
//! rate (one frame per input frame) and doubling it (one frame per field, e.g.
//! 50i → 50p); `field_order` fixes mis-flagged files that judder; `apply_to`
//! restricts the pass to frames the decoder flagged as interlaced.
//!
//! The picture is rewritten, so the video is re-encoded to H.264 (`-crf 20`);
//! audio is kept as-is (stream-copied), and re-encoded to AAC only when the
//! container must switch to MP4 (e.g. webm).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); source-resolution, ffmpeg dispatch, and
//! envelope-building are delegated to `block_utils`. Param validation and the
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
use gizza_ai_video_deinterlace_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Deinterlacer: bwdif (default) or yadif.
    #[serde(default)]
    filter: Option<String>,
    /// frame (keep fps, default) or field (double fps).
    #[serde(default)]
    mode: Option<String>,
    /// auto (default), tff or bff.
    #[serde(default)]
    field_order: Option<String>,
    /// all (default) or flagged.
    #[serde(default)]
    apply_to: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("filter", ["bwdif", "yadif"])
                .default("bwdif")
                .describe("Deinterlacer to run: bwdif (Bob Weaver, motion-adaptive with a sharper interpolator — default, best detail) or yadif (the classic ffmpeg deinterlacer, slightly softer but the most widely documented)."),
        )
        .param(
            Param::enumv("mode", ["frame", "field"])
                .default("frame")
                .describe("How fields become frames: frame = one output frame per input frame, frame rate unchanged (50i → 25p; default), or field = one output frame per field, which DOUBLES the frame rate (50i → 50p) and restores the original smooth broadcast motion at ~2x the frames/file size."),
        )
        .param(
            Param::enumv("field_order", ["auto", "tff", "bff"])
                .default("auto")
                .describe("Which field is shown first: auto trusts the flags in the file (default, right for almost every capture), tff = top field first (DV/HDV, 1080i broadcast), bff = bottom field first (most SD DVD/analogue captures). Force tff/bff if the deinterlaced motion judders or jerks backwards — that means the flags are wrong."),
        )
        .param(
            Param::enumv("apply_to", ["all", "flagged"])
                .default("all")
                .describe("Which frames to deinterlace: all = every frame (default; use this when the file has no interlaced flags, which is common for captures and re-encodes), or flagged = only frames the decoder marked as interlaced, leaving progressive frames in mixed footage untouched."),
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
struct VideoDeinterlace;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-deinterlace",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove interlacing combing from a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Remove interlacing combing artifacts (the horizontal comb teeth on moving edges in camcorder, DV, DVD and broadcast footage) and write clean progressive frames, using ffmpeg's motion-adaptive deinterlacers. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call), plus filter (bwdif = default, sharper; or yadif = classic), mode (frame = keep the frame rate, default; field = one frame per field, doubling the frame rate, e.g. 50i → 50p), field_order (auto = trust the file's flags, default; tff or bff to force it when the motion judders) and apply_to (all frames, default; or flagged = only frames marked interlaced). The picture is re-encoded to H.264 (crf 20) and flagged progressive; audio is kept as-is. mp4/mov/m4v/mkv inputs keep their container; other inputs (e.g. webm) are converted to MP4. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoDeinterlace {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args. Unset params fall back to the core defaults (the page
    //    sends "" for an untouched select, which core treats the same way).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-deinterlace")?;
    let filter = args.filter.as_deref().unwrap_or("bwdif");
    let mode = args.mode.as_deref().unwrap_or("frame");
    let field_order = args.field_order.as_deref().unwrap_or("auto");
    let apply_to = args.apply_to.as_deref().unwrap_or("all");

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates every enum param).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(filter, mode, field_order, apply_to, &ffmpeg_in)
        .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-deinterlaced", out_ext);
    let rate = if mode == "field" {
        "frame rate doubled"
    } else {
        "frame rate kept"
    };
    let for_llm = format!(
        "deinterlaced {in_filename} ({filter}, {mode} mode — {rate}, field order {field_order}, applied to {apply_to} frames) ({output_size} bytes {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + the four enum params), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":         { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "filter":      { "type": "string", "enum": ["bwdif", "yadif"], "default": "bwdif", "description": "Deinterlacer to run: bwdif (Bob Weaver, motion-adaptive with a sharper interpolator — default, best detail) or yadif (the classic ffmpeg deinterlacer, slightly softer but the most widely documented)." },
                    "mode":        { "type": "string", "enum": ["frame", "field"], "default": "frame", "description": "How fields become frames: frame = one output frame per input frame, frame rate unchanged (50i → 25p; default), or field = one output frame per field, which DOUBLES the frame rate (50i → 50p) and restores the original smooth broadcast motion at ~2x the frames/file size." },
                    "field_order": { "type": "string", "enum": ["auto", "tff", "bff"], "default": "auto", "description": "Which field is shown first: auto trusts the flags in the file (default, right for almost every capture), tff = top field first (DV/HDV, 1080i broadcast), bff = bottom field first (most SD DVD/analogue captures). Force tff/bff if the deinterlaced motion judders or jerks backwards — that means the flags are wrong." },
                    "apply_to":    { "type": "string", "enum": ["all", "flagged"], "default": "all", "description": "Which frames to deinterlace: all = every frame (default; use this when the file has no interlaced flags, which is common for captures and re-encodes), or flagged = only frames the decoder marked as interlaced, leaving progressive frames in mixed footage untouched." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The descriptor's advertised enum values must be exactly the ones core
    /// accepts — a value the schema offers but core rejects would be a dead
    /// choice in the chat/CLI/page dropdown.
    #[test]
    fn every_advertised_enum_value_is_accepted_by_core() {
        for filter in ["bwdif", "yadif"] {
            for mode in ["frame", "field"] {
                for order in ["auto", "tff", "bff"] {
                    for apply in ["all", "flagged"] {
                        assert!(
                            plan(filter, mode, order, apply, "in.mp4").is_ok(),
                            "{filter}/{mode}/{order}/{apply} advertised but rejected"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_enum_value_is_a_clear_error() {
        let err = plan("nnedi", "frame", "auto", "all", "in.mp4").unwrap_err();
        assert!(err.contains("bwdif|yadif"), "{err}");
    }

    #[test]
    fn output_filename_uses_deinterlaced_suffix() {
        assert_eq!(
            filename_with_suffix("tape.mp4", "-deinterlaced", "mp4"),
            "tape-deinterlaced.mp4"
        );
    }
}
