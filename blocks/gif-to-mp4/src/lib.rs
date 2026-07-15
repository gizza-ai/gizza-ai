//! gizza-ai/gif-to-mp4 — fetch an animated GIF, convert it to a smaller MP4/WebM
//! video via ffmpeg, return an envelope. Source-resolution, ffmpeg dispatch, and
//! envelope-building are delegated to `block_utils`; the pure argv builder lives
//! in `core`. NOTE: chat ffmpeg is non-functional (Service Worker) — page + CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SkillResultExt,
    ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_gif_to_mp4_core::{build_argv, parse_format};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::enumv("format", ["mp4", "webm"])
            .default("mp4")
            .describe("Output video format: mp4 (H.264, default) or webm (VP9)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GifToMp4;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gif-to-mp4",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an animated GIF to a smaller MP4/WebM video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert an animated GIF into a much smaller MP4 (H.264) or WebM (VP9) video, preserving the animation. Set format='mp4' (default) or 'webm'. Provide the GIF as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl GifToMp4 {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("gif-to-mp4")?;
    let fmt = parse_format(args.format.as_deref().unwrap_or("mp4")).invalid_args("gif-to-mp4")?;

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("gif");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = build_argv(&ffmpeg_in, fmt);

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_mime = if fmt == "webm" { "video/webm" } else { "video/mp4" };
    let output_size = output.len();
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(&in_filename);
    let filename = format!("{stem}.{fmt}");
    let for_llm = format!("converted {in_filename} to {fmt} ({output_size} bytes {out_mime})");
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + format enum).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["mp4", "webm"], "default": "mp4", "description": "Output video format: mp4 (H.264, default) or webm (VP9)." }
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
