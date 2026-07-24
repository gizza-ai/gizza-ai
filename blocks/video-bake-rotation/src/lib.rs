//! gizza-ai/video-bake-rotation — fetch a video URL or attachment ref, bake any
//! embedded rotation metadata/display matrix into actual pixels, clear the rotate
//! flag, and return a media envelope. This fixes players that ignore orientation
//! metadata. The chat schema is derived from `descriptor()`; source-resolution,
//! ffmpeg dispatch, and envelope-building are delegated to `block_utils`.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_bake_rotation_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// No scalar params: the input rotation metadata is the source of truth.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}

#[cfg(target_arch = "wasm32")]
struct VideoBakeRotation;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-bake-rotation",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bake a video's rotation metadata into upright pixels",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Bake a video's embedded rotation metadata/display matrix into the actual pixels and clear the rotate flag, fixing sideways phone clips in players that ignore orientation metadata. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). The video is re-encoded to H.264 with CRF 23/preset medium so the orientation becomes universal; audio is stream-copied when container-compatible, or re-encoded to AAC when converting to mp4. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoBakeRotation {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-bake-rotation")?;
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in).map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-rotation-baked", out_ext);
    let for_llm = format!(
        "baked rotation metadata into pixels for {in_filename} and cleared the rotate flag ({output_size} bytes {out_mime})"
    );
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
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
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
    fn output_filename_uses_rotation_baked_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-rotation-baked", "mp4"),
            "clip-rotation-baked.mp4"
        );
    }
}
