//! gizza-ai/video-strip-metadata — remove global, stream, and chapter metadata
//! from a video by remuxing it with ffmpeg stream copy (no re-encode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_strip_metadata_core::build_argv;
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
    #[serde(default = "default_chapters")]
    chapters: String,
}

fn default_container() -> String { "keep".to_string() }
fn default_chapters() -> String { "remove".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("container", ["keep", "mp4"])
                .default("keep")
                .describe("Output container: keep preserves the input container (default, best compatibility); mp4 remuxes to .mp4 without re-encoding and requires MP4-compatible streams."),
        )
        .param(
            Param::enumv("chapters", ["remove", "keep"])
                .default("remove")
                .describe("Chapter metadata handling: remove drops chapter markers (default); keep preserves them."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-strip-metadata",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove embedded metadata from a video without re-encoding",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Strip embedded metadata from a video by remuxing with ffmpeg stream copy. Removes global tags, per-stream tags, chapters by default, GPS/location/device/timestamp/comment/title-style metadata, and avoids writing a fresh encoder tag. Provide a video as url or ref. Params: container=keep|mp4 (default keep), chapters=remove|keep (default remove). No re-encode, so quality is preserved; forcing mp4 requires MP4-compatible streams.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("video-strip-metadata")?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let (argv, out_name) = build_argv(&args.container, &args.chapters, &format!("in.{ext}"))
        .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, format!("in.{ext}"), bytes, out_name)?;
    let out_ext = if args.container == "mp4" { "mp4" } else { ext };
    let out_mime = ext_to_video_mime(out_ext);
    let filename = filename_with_suffix(&in_name, "-metadata-stripped", out_ext);
    let for_llm = format!(
        "stripped metadata from {in_name} without re-encoding (container={}; chapters={}) -> {filename}",
        args.container, args.chapters
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "container": { "type": "string", "enum": ["keep", "mp4"], "default": "keep", "description": "Output container: keep preserves the input container (default, best compatibility); mp4 remuxes to .mp4 without re-encoding and requires MP4-compatible streams." },
                    "chapters": { "type": "string", "enum": ["remove", "keep"], "default": "remove", "description": "Chapter metadata handling: remove drops chapter markers (default); keep preserves them." }
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
    fn args_default_to_keep_container_remove_chapters() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.mp4"}"#).unwrap();
        assert_eq!(a.container, "keep");
        assert_eq!(a.chapters, "remove");
    }
}
