//! gizza-ai/change-speed — fetch a video URL or attachment ref, change its
//! playback speed via ffmpeg (keeping audio in sync), return an envelope.
//! Source-resolution, ffmpeg dispatch, and envelope-building are delegated to
//! `block_utils`; the pure argv builder lives in `core`.
//!
//! NOTE: chat ffmpeg is non-functional (Service Worker) — page + CLI surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_change_speed_core::{build_argv, MAX_FACTOR, MIN_FACTOR};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    factor: f64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video).param(
        Param::number("factor")
            .required()
            .min(MIN_FACTOR)
            .max(MAX_FACTOR)
            .describe("Speed multiplier: >1 speeds up, <1 slows down (e.g. 2 = twice as fast, 0.5 = half speed). Range 0.25-4."),
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
struct ChangeSpeed;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/change-speed",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Speed up or slow down a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Speed up or slow down a video by a factor, keeping the audio in sync (>1 faster, <1 slower; e.g. 2 = double speed, 0.5 = half). Range 0.25-4. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl ChangeSpeed {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("change-speed")?;
    if !args.factor.is_finite() || args.factor < MIN_FACTOR || args.factor > MAX_FACTOR {
        return Err(SkillError::InvalidArgs(format!(
            "invalid change-speed args: factor must be between {MIN_FACTOR} and {MAX_FACTOR}"
        )));
    }

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{in_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.factor);

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, &format!("-{}x", args.factor), out_ext);
    let for_llm = format!("changed {in_filename} speed by {}x ({output_size} bytes {out_mime})", args.factor);
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + required number `factor`).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "factor": { "type": "number", "minimum": 0.25, "maximum": 4, "description": "Speed multiplier: >1 speeds up, <1 slows down (e.g. 2 = twice as fast, 0.5 = half speed). Range 0.25-4." }
                },
                "required": ["factor"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
