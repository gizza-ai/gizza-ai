//! gizza-ai/video-duration-fix-remux — repair missing or wrong container-level
//! duration metadata by REMUXING a video with ffmpeg stream copy (no re-encode).
//! Input::Video emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
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
    #[serde(default = "default_faststart")]
    faststart: bool,
    #[serde(default)]
    regen_timestamps: bool,
}

fn default_container() -> String { "keep".to_string() }
fn default_faststart() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("container", ["keep", "mp4", "mkv", "mov", "webm"])
                .default("keep")
                .describe("Output container. keep (default) rebuilds the same container as the input, which is the safest lossless fix; mp4/mkv/mov/webm remux into that container instead. Only stream copy is used, so the codecs must be compatible with the chosen container (e.g. H.264 fits mp4/mkv/mov but not webm; VP9/Opus fit webm/mkv/mp4)."),
        )
        .param(
            Param::boolean("faststart")
                .default(true)
                .describe("MP4/MOV output only: move the moov atom (index) to the front of the file so players read the correct duration immediately and the file streams progressively (-movflags +faststart). Ignored for mkv/webm. Default true."),
        )
        .param(
            Param::boolean("regen_timestamps")
                .default(false)
                .describe("Regenerate missing or broken presentation timestamps before remuxing (-fflags +genpts). Enable when the duration reads as 0, N/A, or Infinity (typical of MediaRecorder/screen-capture WebM). Default false."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-duration-fix-remux",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Repair missing or wrong video duration metadata by remuxing without re-encoding",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Repair missing or wrong container-level duration metadata by remuxing a video with ffmpeg stream copy (-c copy). Remuxing parses the packets and writes a fresh container header, so the duration/index becomes correct without re-encoding — quality is preserved bit-for-bit. Fixes MediaRecorder/screen-capture WebM whose duration reads as Infinity, MP4/MOV with a broken moov atom, and clips whose header duration disagrees with the real length. Provide a video as url or ref. Params: container=keep|mp4|mkv|mov|webm (default keep), faststart=true|false (MP4/MOV moov-to-front, default true), regen_timestamps=true|false (add -fflags +genpts for broken PTS, default false). Only stream copy, so the codecs must be compatible with the chosen container.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("video-duration-fix-remux")?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let (argv, out_name) = gizza_ai_video_duration_fix_remux_core::plan(
        &args.container,
        args.faststart,
        args.regen_timestamps,
        &format!("in.{ext}"),
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, format!("in.{ext}"), bytes, out_name)?;
    let out_ext = if args.container == "keep" { ext } else { args.container.as_str() };
    let out_mime = ext_to_video_mime(out_ext);
    let filename = filename_with_suffix(&in_name, "-duration-fixed", out_ext);
    let for_llm = format!(
        "remuxed {in_name} to repair duration metadata without re-encoding (container={}; faststart={}; regen_timestamps={}) -> {filename}",
        args.container, args.faststart, args.regen_timestamps
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
                    "container": { "type": "string", "enum": ["keep", "mp4", "mkv", "mov", "webm"], "default": "keep", "description": "Output container. keep (default) rebuilds the same container as the input, which is the safest lossless fix; mp4/mkv/mov/webm remux into that container instead. Only stream copy is used, so the codecs must be compatible with the chosen container (e.g. H.264 fits mp4/mkv/mov but not webm; VP9/Opus fit webm/mkv/mp4)." },
                    "faststart": { "type": "boolean", "default": true, "description": "MP4/MOV output only: move the moov atom (index) to the front of the file so players read the correct duration immediately and the file streams progressively (-movflags +faststart). Ignored for mkv/webm. Default true." },
                    "regen_timestamps": { "type": "boolean", "default": false, "description": "Regenerate missing or broken presentation timestamps before remuxing (-fflags +genpts). Enable when the duration reads as 0, N/A, or Infinity (typical of MediaRecorder/screen-capture WebM). Default false." }
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
    fn args_default_to_keep_faststart_on_regen_off() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.webm"}"#).unwrap();
        assert_eq!(a.container, "keep");
        assert!(a.faststart);
        assert!(!a.regen_timestamps);
    }
}
