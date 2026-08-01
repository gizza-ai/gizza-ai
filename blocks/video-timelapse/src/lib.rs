//! gizza-ai/video-timelapse — fetch a video URL or attachment ref and turn it
//! into a timelapse: speed it up by `speed` (`setpts=PTS/speed`) and re-sample
//! to a fixed `fps`, DROPPING the surplus frames the speed-up crammed in. Audio
//! is always dropped (a 20×-fast soundtrack is noise). The video is re-encoded
//! to H.264 (`-crf 20`, `yuv420p`, `+faststart`) for universal playback. The
//! container is kept for inputs that can hold H.264 (mp4/mov/m4v/mkv); anything
//! else (e.g. webm) is converted to MP4 — see `h264_out_ext`.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Tool-specific validation
//! (finite `speed`/`fps`) and the pure `core` argv builder stay in `core`.

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
use gizza_ai_video_timelapse_core::{build_argv, resolve_fps, resolve_speed};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Speed-up factor; omitted → core default (10×).
    #[serde(default)]
    speed: Option<f64>,
    /// Output frame rate; omitted → core default (30 fps).
    #[serde(default)]
    fps: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("speed")
                .min(2.0)
                .max(300.0)
                .describe("How many times faster the timelapse plays, e.g. 10 = 10× faster (a 60s clip becomes 6s). Higher = more footage compressed. Default 10. Clamped to 2-300."),
        )
        .param(
            Param::number("fps")
                .min(1.0)
                .max(60.0)
                .describe("Output frame rate in frames per second (e.g. 30, 24, 60). The sped-up video is re-sampled to this rate, dropping surplus frames. Default 30. Clamped to 1-60."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoTimelapse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-timelapse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn long footage into a timelapse by dropping frames and speeding it up",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Turn long footage into a timelapse by speeding it up and dropping frames (audio is dropped). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call), plus speed (how many times faster, e.g. 10 = 10×, default 10) and fps (output frame rate, default 30). The clip is sped up with setpts and re-sampled to the target fps, dropping the surplus frames — a 60s clip at 10× becomes 6s. The video is re-encoded to H.264 (crf 20, yuv420p, faststart); audio is removed. mp4/mov/m4v/mkv inputs keep their container; other inputs (e.g. webm) are converted to MP4. speed is clamped to 2-300, fps to 1-60.",
        parameters = schema_json()
    ),
)]
impl VideoTimelapse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args. Reject an explicit non-finite speed/fps up front (a clear
    //    user error); unset/out-of-range is resolved by core.
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-timelapse args: {e}")))?;
    if let Some(s) = args.speed {
        if !s.is_finite() {
            return Err(SkillError::InvalidArgs(
                "invalid video-timelapse args: speed must be a finite number".into(),
            ));
        }
    }
    if let Some(f) = args.fps {
        if !f.is_finite() {
            return Err(SkillError::InvalidArgs(
                "invalid video-timelapse args: fps must be a finite number".into(),
            ));
        }
    }
    // The page's "unset" sentinel is 0.0; the chat schema omits the field
    // instead, so map None → 0.0 to hit core's default path.
    let speed_req = args.speed.unwrap_or(0.0);
    let fps_req = args.fps.unwrap_or(0.0);
    let speed = resolve_speed(speed_req);
    let fps = resolve_fps(fps_req);

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (core keeps the input container when it can hold
    //    H.264, otherwise switches to mp4 — see h264_out_ext).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = build_argv(speed_req, fps_req, &ffmpeg_in);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope. Output mime follows the produced extension.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-timelapse", out_ext);
    let for_llm = format!(
        "timelapsed {in_filename} at {speed}× to {fps} fps ({output_size} bytes {out_mime})"
    );
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
                    "speed": { "type": "number", "minimum": 2, "maximum": 300, "description": "How many times faster the timelapse plays, e.g. 10 = 10× faster (a 60s clip becomes 6s). Higher = more footage compressed. Default 10. Clamped to 2-300." },
                    "fps": { "type": "number", "minimum": 1, "maximum": 60, "description": "Output frame rate in frames per second (e.g. 30, 24, 60). The sped-up video is re-sampled to this rate, dropping surplus frames. Default 30. Clamped to 1-60." }
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
    fn output_filename_uses_timelapse_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-timelapse", "mp4"),
            "clip-timelapse.mp4"
        );
    }
}
