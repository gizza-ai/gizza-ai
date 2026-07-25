//! gizza-ai/video-mute-section — fetch a video URL or attachment ref, silence
//! the audio over ONE chosen `[start, end]` time range while leaving the rest
//! of the soundtrack intact, and return an envelope. The picture is
//! stream-copied (lossless, untouched); only the audio is re-encoded (its
//! samples inside the window are being zeroed). The chat schema is derived from
//! `descriptor()` (single source — shared across chat + CLI + page);
//! source-resolution, ffmpeg dispatch, and envelope-building are delegated to
//! `block_utils`. Window validation and the pure argv builder live in `core`.
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
use gizza_ai_video_mute_section_core::{fmt_num, plan};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    start: f64,
    end: f64,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("start")
                .required()
                .min(0.0)
                .describe("Start of the silenced range, in seconds from the beginning of the video (e.g. 5 or 12.5). Must be >= 0 and less than end."),
        )
        .param(
            Param::number("end")
                .required()
                .min(0.0)
                .describe("End of the silenced range, in seconds from the beginning of the video (e.g. 10 or 18.25). Must be greater than start; the audio between start and end is silenced, everything else is untouched."),
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
struct VideoMuteSection;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-mute-section",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Silence a video's audio over a chosen time range, keeping the rest",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Silence the audio over one chosen [start, end] time range of a video while leaving the rest of the soundtrack intact, without touching the picture (the video stream is copied losslessly; only the audio is re-encoded). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). start and end are seconds from the beginning of the clip; end must be greater than start. The output keeps the input container (mp4→mp4, webm→webm; webm audio becomes Opus, otherwise AAC). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoMuteSection {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; window validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-mute-section")?;

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates the window).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, args.start, args.end).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-muted", out_ext);
    let for_llm = format!(
        "silenced the audio of {in_filename} between {} s and {} s (rest of the soundtrack and the picture untouched, {output_size} bytes {out_mime})",
        fmt_num(args.start),
        fmt_num(args.end)
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + start/end), so any future change to
    /// the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":   { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":   { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "start": { "type": "number", "minimum": 0, "description": "Start of the silenced range, in seconds from the beginning of the video (e.g. 5 or 12.5). Must be >= 0 and less than end." },
                    "end":   { "type": "number", "minimum": 0, "description": "End of the silenced range, in seconds from the beginning of the video (e.g. 10 or 18.25). Must be greater than start; the audio between start and end is silenced, everything else is untouched." }
                },
                "required": ["start", "end"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_muted_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-muted", "mp4"),
            "clip-muted.mp4"
        );
    }
}
