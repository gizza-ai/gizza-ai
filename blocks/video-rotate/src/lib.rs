//! gizza-ai/video-rotate — rotate and/or flip a video via ffmpeg.
//! Delegates source-resolution, ffmpeg dispatch, and envelope-building to
//! block_utils. NOTE: chat ffmpeg is non-functional — page + CLI surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_rotate_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    rotate: u32,
    #[serde(default)]
    flip: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(Param::integer("rotate").default(0).min(0.0).max(270.0).describe("Clockwise rotation in degrees: 0, 90, 180, or 270. Default 0."))
        .param(Param::enumv("flip", ["none", "horizontal", "vertical"]).default("none").describe("Optional flip/mirror. Default none."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext { "webm" => "video/webm", "mov" => "video/quicktime", "mkv" => "video/x-matroska", _ => "video/mp4" }
}

#[cfg(target_arch = "wasm32")]
struct VideoRotate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-rotate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rotate and/or flip a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Rotate a video clockwise by 90, 180, or 270 degrees and/or flip it horizontally or vertically. Set rotate (0/90/180/270) and/or flip (none/horizontal/vertical); at least one must be active. Provide the video as either url (HTTP/HTTPS) or ref. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoRotate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-rotate")?;
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    // plan() validates rotate/flip and that at least one is active, and picks
    // out.<ext> — the input container when it can hold H.264+AAC, otherwise
    // mp4 with the audio re-encoded to AAC (e.g. webm input).
    let (argv, ffmpeg_out) = plan(&ffmpeg_in, args.rotate, &args.flip).map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-rotated", out_ext);
    let for_llm = format!("rotated {in_filename} ({output_size} bytes {out_mime})");
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
                    "rotate": { "type": "integer", "minimum": 0, "maximum": 270, "default": 0, "description": "Clockwise rotation in degrees: 0, 90, 180, or 270. Default 0." },
                    "flip":   { "type": "string", "enum": ["none", "horizontal", "vertical"], "default": "none", "description": "Optional flip/mirror. Default none." }
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
