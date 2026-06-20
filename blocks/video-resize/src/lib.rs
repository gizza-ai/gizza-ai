//! gizza-ai/video-resize — scale a video to a target resolution via ffmpeg.
//! Source-resolution, ffmpeg dispatch, and envelope-building delegated to
//! block_utils. NOTE: chat ffmpeg is non-functional — page + CLI surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_resize_core::build_argv;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(Param::integer("width").min(1.0).describe("Target width in pixels. Omit to scale by height, preserving aspect ratio."))
        .param(Param::integer("height").min(1.0).describe("Target height in pixels. Omit to scale by width, preserving aspect ratio."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext { "webm" => "video/webm", "mov" => "video/quicktime", "mkv" => "video/x-matroska", _ => "video/mp4" }
}

#[cfg(target_arch = "wasm32")]
struct VideoResize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-resize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scale a video to a target resolution",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Scale a video to a target resolution. Give width and/or height in pixels; omit one to preserve the aspect ratio (the omitted side is computed to an even number). Provide the video as either url (HTTP/HTTPS) or ref. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoResize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-resize")?;
    if args.width.is_none() && args.height.is_none() {
        return Err(SkillError::InvalidArgs("invalid video-resize args: at least one of width/height required".into()));
    }
    if args.width == Some(0) || args.height == Some(0) {
        return Err(SkillError::InvalidArgs("invalid video-resize args: width/height must be > 0".into()));
    }
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{in_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.width, args.height);
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let suffix = match (args.width, args.height) {
        (Some(w), Some(h)) => format!("-{w}x{h}"),
        _ => "-resized".to_string(),
    };
    let filename = filename_with_suffix(&in_filename, &suffix, out_ext);
    let for_llm = format!("resized {in_filename} ({output_size} bytes {out_mime})");
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "width":  { "type": "integer", "minimum": 1, "description": "Target width in pixels. Omit to scale by height, preserving aspect ratio." },
                    "height": { "type": "integer", "minimum": 1, "description": "Target height in pixels. Omit to scale by width, preserving aspect ratio." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
