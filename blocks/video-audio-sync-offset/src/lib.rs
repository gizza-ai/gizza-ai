//! gizza-ai/video-audio-sync-offset — fetch a video URL or attachment ref,
//! shift its audio track earlier or later relative to the picture by a fixed
//! time offset (to fix lip-sync drift), and return an envelope. The picture is
//! stream-copied (lossless); only the audio is re-encoded (the timeline is
//! rewritten). The chat schema is derived from `descriptor()` (single source —
//! shared across chat + CLI + page); source-resolution, ffmpeg dispatch, and
//! envelope-building are delegated to `block_utils`. Offset/unit validation and
//! the pure argv builder live in `core`.
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
use gizza_ai_video_audio_sync_offset_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    offset: f64,
    #[serde(default)]
    unit: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("offset")
                .required()
                .min(-60000.0)
                .max(60000.0)
                .describe("How far to shift the audio relative to the picture. Positive DELAYS the audio (plays later — fixes audio that runs ahead of the video); negative ADVANCES it (plays earlier — fixes audio that lags). In the chosen unit; range ±60000 ms (±60 s); 0 not allowed. Example: 200 with unit=ms, or 0.2 with unit=seconds."),
        )
        .param(
            Param::enumv("unit", ["ms", "seconds"])
                .default("ms")
                .describe("How offset is interpreted: milliseconds (default) or seconds. 200 ms = 0.2 seconds."),
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
struct VideoAudioSyncOffset;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-sync-offset",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shift a video's audio earlier or later to fix lip-sync",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Shift a video's audio track earlier or later relative to the picture by a fixed time offset, to correct lip-sync drift. The picture is kept untouched (the video stream is copied losslessly; only the audio is re-encoded). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). offset is in milliseconds by default (200 delays the audio, -200 advances it; range ±60000) or in seconds with unit=seconds (0.2, -0.2). A positive offset delays the audio (plays later, fixing audio that runs ahead); a negative offset advances it (plays earlier, fixing audio that lags). The output keeps the input container. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioSyncOffset {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; offset/unit validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-sync-offset")?;
    let unit = args.unit.as_deref().unwrap_or("ms");

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates offset for the unit).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, args.offset, unit).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-synced", out_ext);
    let unit_label = if unit == "seconds" { "s" } else { "ms" };
    let direction = if args.offset >= 0.0 {
        "delayed"
    } else {
        "advanced"
    };
    let for_llm = format!(
        "{direction} the audio of {in_filename} by {} {unit_label} relative to the video ({output_size} bytes {out_mime})",
        args.offset.abs()
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + offset/unit), so any future change
    /// to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "offset": { "type": "number", "minimum": -60000, "maximum": 60000, "description": "How far to shift the audio relative to the picture. Positive DELAYS the audio (plays later — fixes audio that runs ahead of the video); negative ADVANCES it (plays earlier — fixes audio that lags). In the chosen unit; range ±60000 ms (±60 s); 0 not allowed. Example: 200 with unit=ms, or 0.2 with unit=seconds." },
                    "unit":   { "type": "string", "enum": ["ms", "seconds"], "default": "ms", "description": "How offset is interpreted: milliseconds (default) or seconds. 200 ms = 0.2 seconds." }
                },
                "required": ["offset"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_synced_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-synced", "mp4"),
            "clip-synced.mp4"
        );
    }
}
