//! gizza-ai/video-aspect-ratio-fix — rewrite the display aspect ratio (DAR) tag
//! on a video whose stored aspect metadata is wrong, using ffmpeg stream copy
//! (no re-encode). Input::Video emits a url⊕ref oneOf; run() uses
//! resolve_source → core::plan → dispatch_ffmpeg → build_media_envelope.
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
    #[serde(default = "default_aspect")]
    aspect: String,
    #[serde(default)]
    custom_aspect: String,
    #[serde(default = "default_container")]
    container: String,
    #[serde(default = "default_faststart")]
    faststart: bool,
}

fn default_aspect() -> String { "16:9".to_string() }
fn default_container() -> String { "keep".to_string() }
fn default_faststart() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv(
                "aspect",
                [
                    "16:9", "9:16", "4:3", "3:4", "1:1", "21:9", "2.39:1", "1.85:1", "5:4", "4:5",
                    "3:2", "2:3", "custom",
                ],
            )
            .default("16:9")
            .describe("Display aspect ratio (DAR) to tag the video with. 16:9 (default) is standard widescreen, 9:16 vertical/phone, 4:3 classic TV, 1:1 square, 21:9 ultrawide, 2.39:1 and 1.85:1 cinema. Pick custom to supply your own ratio in custom_aspect. The stored pixel size is never changed — only the shape the player is told to display."),
        )
        .param(
            Param::string("custom_aspect")
                .default("")
                .describe("Your own ratio, used ONLY when aspect=custom (ignored otherwise). Accepts 'W:H' (16:9), 'W/H' (16/9), a decimal width÷height (1.85), or display dimensions 'WxH' (1920x1080). Must work out to between 0.05 and 20. Tip: to reset a file to square pixels, pass its stored pixel size, e.g. 640x480."),
        )
        .param(
            Param::enumv("container", ["keep", "mp4", "mkv", "mov", "webm"])
                .default("keep")
                .describe("Output container. keep (default) rebuilds the same container as the input, which is the safest lossless fix; mp4/mkv/mov/webm remux into that container instead. Only stream copy is used, so the codecs must be compatible with the chosen container (e.g. H.264/AAC fits mp4/mov/mkv but not webm; VP9/Opus fit webm/mkv/mp4)."),
        )
        .param(
            Param::boolean("faststart")
                .default(true)
                .describe("MP4/MOV output only: move the moov atom (index) to the front of the file so players read the new aspect ratio immediately and the file streams progressively (-movflags +faststart). Ignored for mkv/webm. Default true."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-aspect-ratio-fix",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fix a stretched or squashed video by retagging its display aspect ratio without re-encoding",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Fix a video that plays stretched, squashed, or anamorphic because its stored aspect metadata is wrong. Rewrites the display aspect ratio (DAR) tag with ffmpeg stream copy (-map 0 -c copy -aspect W:H): the container header is rebuilt with the correct ratio while the audio and video packets are copied bit-for-bit, so there is no re-encode and no quality loss. The frame's stored pixel size is unchanged — only the shape the player displays. Provide a video as url or ref. Params: aspect=16:9|9:16|4:3|3:4|1:1|21:9|2.39:1|1.85:1|5:4|4:5|3:2|2:3|custom (default 16:9), custom_aspect (used only when aspect=custom; accepts 16:9, 16/9, 1.85 or 1920x1080), container=keep|mp4|mkv|mov|webm (default keep), faststart=true|false (MP4/MOV moov-to-front, default true). To reset a file to square pixels, use aspect=custom with the video's stored pixel size, e.g. 640x480. To letterbox, crop, or actually rescale the pixels, use the video-aspect-pad, video-crop, or video-resize tools instead.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("video-aspect-ratio-fix")?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let (argv, out_name) = gizza_ai_video_aspect_ratio_fix_core::plan(
        &args.aspect,
        &args.custom_aspect,
        &args.container,
        args.faststart,
        &format!("in.{ext}"),
    )
    .map_err(SkillError::InvalidArgs)?;
    let (num, den) =
        gizza_ai_video_aspect_ratio_fix_core::resolve_aspect(&args.aspect, &args.custom_aspect)
            .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, format!("in.{ext}"), bytes, out_name)?;
    let out_ext = if args.container == "keep" { ext } else { args.container.as_str() };
    let out_mime = ext_to_video_mime(out_ext);
    let filename = filename_with_suffix(&in_name, "-aspect-fixed", out_ext);
    let for_llm = format!(
        "retagged {in_name} with display aspect ratio {num}:{den} using stream copy — no re-encode, pixels unchanged (container={}; faststart={}) -> {filename}",
        args.container, args.faststart
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
                    "aspect": { "type": "string", "enum": ["16:9", "9:16", "4:3", "3:4", "1:1", "21:9", "2.39:1", "1.85:1", "5:4", "4:5", "3:2", "2:3", "custom"], "default": "16:9", "description": "Display aspect ratio (DAR) to tag the video with. 16:9 (default) is standard widescreen, 9:16 vertical/phone, 4:3 classic TV, 1:1 square, 21:9 ultrawide, 2.39:1 and 1.85:1 cinema. Pick custom to supply your own ratio in custom_aspect. The stored pixel size is never changed — only the shape the player is told to display." },
                    "custom_aspect": { "type": "string", "default": "", "description": "Your own ratio, used ONLY when aspect=custom (ignored otherwise). Accepts 'W:H' (16:9), 'W/H' (16/9), a decimal width÷height (1.85), or display dimensions 'WxH' (1920x1080). Must work out to between 0.05 and 20. Tip: to reset a file to square pixels, pass its stored pixel size, e.g. 640x480." },
                    "container": { "type": "string", "enum": ["keep", "mp4", "mkv", "mov", "webm"], "default": "keep", "description": "Output container. keep (default) rebuilds the same container as the input, which is the safest lossless fix; mp4/mkv/mov/webm remux into that container instead. Only stream copy is used, so the codecs must be compatible with the chosen container (e.g. H.264/AAC fits mp4/mov/mkv but not webm; VP9/Opus fit webm/mkv/mp4)." },
                    "faststart": { "type": "boolean", "default": true, "description": "MP4/MOV output only: move the moov atom (index) to the front of the file so players read the new aspect ratio immediately and the file streams progressively (-movflags +faststart). Ignored for mkv/webm. Default true." }
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
    fn args_default_to_16_9_keep_container_faststart_on() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.mp4"}"#).unwrap();
        assert_eq!(a.aspect, "16:9");
        assert_eq!(a.custom_aspect, "");
        assert_eq!(a.container, "keep");
        assert!(a.faststart);
    }

    #[test]
    fn custom_args_reach_core_and_normalize() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.mp4","aspect":"custom","custom_aspect":"1920x1080"}"#).unwrap();
        let (argv, _) =
            gizza_ai_video_aspect_ratio_fix_core::plan(&a.aspect, &a.custom_aspect, &a.container, a.faststart, "in.mp4")
                .unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-aspect" && w[1] == "16:9"));
    }
}
