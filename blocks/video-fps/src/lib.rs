//! gizza-ai/video-fps — fetch a video URL or attachment ref and change its
//! frame rate to a fixed target (e.g. 60→30, or any chosen fps) with frame
//! drop/duplication via ffmpeg's `fps` filter. The clip's duration is
//! unchanged; only the frames-per-second changes. The container is kept for
//! inputs that can hold H.264/AAC (mp4/mov/m4v/mkv); anything else (e.g. webm)
//! is converted to MP4 — see `h264_out_ext`.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (a finite `fps`) and the pure `core` argv builder stay here.

// The #[wafer_block] macro emits the impl gated to wasm32; the supporting
// imports/constants/Args type are only used inside that gate, so they appear
// "unused" under native unit tests. `descriptor()`/`schema_json()` stay
// native-compilable so the drift-guard test can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_fps_core::{build_argv, resolve_fps};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Target frame rate; omitted → core default (30 fps).
    #[serde(default)]
    fps: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video).param(
        Param::number("fps")
            .min(1.0)
            .max(240.0)
            .describe("Target frame rate in frames per second (e.g. 30, 24, 60). Lowering drops frames, raising duplicates them; the clip's duration is unchanged. Default 30. Clamped to 1-240."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoFps;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-fps",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Change a video's frame rate with frame drop/duplication",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Change a video's frame rate to a fixed target (e.g. 60→30, or any chosen fps). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call), plus fps (target frames per second, default 30). Lowering the rate drops frames; raising it duplicates them — the clip's duration is unchanged, only its frames-per-second. The video is re-encoded to H.264 (crf 20); audio is kept as-is. mp4/mov/m4v/mkv inputs keep their container; other inputs (e.g. webm) are converted to MP4. fps is clamped to 1-240.",
        parameters = schema_json()
    ),
)]
impl VideoFps {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args. Reject an explicit non-finite fps up front (a clear user
    //    error); unset/out-of-range is resolved by core.
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-fps args: {e}")))?;
    if let Some(f) = args.fps {
        if !f.is_finite() {
            return Err(SkillError::InvalidArgs(
                "invalid video-fps args: fps must be a finite number".into(),
            ));
        }
    }
    // The page's "unset" sentinel is 0.0; the chat schema omits `fps` instead, so
    // map None → 0.0 to hit core's default path.
    let fps_req = args.fps.unwrap_or(0.0);
    let fps = resolve_fps(fps_req);

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (core keeps the input container when it can hold
    //    H.264/AAC, otherwise switches to mp4 — see h264_out_ext).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = build_argv(fps_req, &ffmpeg_in);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope. Output mime follows the produced extension.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-fps", out_ext);
    let for_llm =
        format!("re-timed {in_filename} to {fps} fps ({output_size} bytes {out_mime})");
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

/// Map an output container extension to its video MIME (mirrors `mime_to_ext`).
#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match this authored
    /// shape so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "fps": { "type": "number", "minimum": 1, "maximum": 240, "description": "Target frame rate in frames per second (e.g. 30, 24, 60). Lowering drops frames, raising duplicates them; the clip's duration is unchanged. Default 30. Clamped to 1-240." }
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
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_fps_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-fps", "mp4"),
            "clip-fps.mp4"
        );
    }
}
