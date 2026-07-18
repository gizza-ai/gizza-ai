//! gizza-ai/video-remove-embedded-subs — strip all embedded subtitle/caption
//! streams from a video by remuxing it with ffmpeg stream copy (no re-encode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_remove_embedded_subs_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_container")]
    container: String,
}

fn default_container() -> String {
    "keep".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video).param(
        Param::enumv("container", ["keep", "mp4", "mkv"])
            .default("keep")
            .describe(
                "Output container: keep preserves the input container/extension (default, best \
                 compatibility); mp4 remuxes to .mp4 (requires MP4-compatible streams); mkv \
                 remuxes to Matroska .mkv (accepts nearly any codec). No re-encode.",
            ),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-remove-embedded-subs",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove all embedded subtitle streams from a video without re-encoding",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Strip every embedded subtitle/caption stream from a video by remuxing with ffmpeg stream copy, keeping the video and audio streams intact. Uses -map 0 -map -0:s -sn -c copy so soft (stream-based) subtitles are dropped while attachments and data streams are preserved. Provide a video as url or ref. Param: container=keep|mp4|mkv (default keep). No re-encode, so quality is preserved. Only soft subtitles are removed — hardcoded/burned-in subtitles are baked into the pixels and cannot be removed by remuxing.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-remove-embedded-subs")?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let (argv, out_name) =
        plan(&args.container, &format!("in.{ext}")).map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, format!("in.{ext}"), bytes, out_name.clone())?;
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or(&ext);
    let out_mime = ext_to_mime(out_ext).unwrap_or(&mime);
    let display_name = filename_with_suffix(&in_name, "-no-subs", out_ext);
    build_media_envelope(
        &output,
        out_mime,
        display_name.clone(),
        format!("subtitle-free video {display_name}"),
        MAX_OUTPUT_BYTES,
    )
}

/// Minimal container-extension → mime lookup for the remux output.
#[cfg(target_arch = "wasm32")]
fn ext_to_mime(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "mp4" | "m4v" => Some("video/mp4"),
        "mkv" => Some("video/x-matroska"),
        "webm" => Some("video/webm"),
        "mov" => Some("video/quicktime"),
        "avi" => Some("video/x-msvideo"),
        _ => None,
    }
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
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "container": { "type": "string", "enum": ["keep", "mp4", "mkv"], "default": "keep", "description": "Output container: keep preserves the input container/extension (default, best compatibility); mp4 remuxes to .mp4 (requires MP4-compatible streams); mkv remuxes to Matroska .mkv (accepts nearly any codec). No re-encode." }
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
    fn args_default_to_keep_container() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.mkv"}"#).unwrap();
        assert_eq!(a.container, "keep");
    }
}
