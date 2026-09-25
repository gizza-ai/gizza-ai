//! gizza-ai/video-reverse — fetch a video URL or attachment ref, reverse its
//! picture/audio with ffmpeg, and return a playable MP4 envelope.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg cannot load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_reverse_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    audio: Option<String>,
    #[serde(default)]
    quality: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("mode", ["reverse", "forward-reverse", "reverse-forward"])
                .default("reverse")
                .describe("Playback shape for the output video. reverse plays the clip backwards. forward-reverse makes a boomerang that plays forward then backward. reverse-forward starts with the reversed half, then returns forward."),
        )
        .param(
            Param::enumv("audio", ["reverse", "keep", "mute"])
                .default("reverse")
                .describe("How to handle sound: reverse matches the backwards picture, keep leaves a forward-playing copy of the original sound, and mute removes the audio track."),
        )
        .param(
            Param::enumv("quality", ["high", "balanced", "small"])
                .default("balanced")
                .describe("H.264 output quality/size preset. high uses CRF 18, balanced uses CRF 23, and small uses CRF 28. Reversing always re-encodes because ffmpeg must reorder frames."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoReverse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-reverse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reverse a video clip, with audio options",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Reverse a video clip locally with ffmpeg. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). mode controls straight reverse or boomerang output (reverse|forward-reverse|reverse-forward). audio controls the soundtrack (reverse|keep|mute). quality controls H.264 CRF (high|balanced|small). Output is MP4. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoReverse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-reverse")?;
    let mode = args.mode.as_deref().unwrap_or("reverse");
    let audio = args.audio.as_deref().unwrap_or("reverse");
    let quality = args.quality.as_deref().unwrap_or("balanced");

    let (input_bytes, _in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ffmpeg_in = "in.mp4".to_string();
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, mode, audio, quality).map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let filename = filename_with_suffix(&in_filename, "-reversed", "mp4");
    let output_size = output.len();
    let for_llm = format!(
        "reversed {in_filename} as {filename} (mode {mode}, audio {audio}, quality {quality}; {output_size} bytes video/mp4)"
    );
    build_media_envelope(&output, "video/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":    { "type": "string", "enum": ["reverse", "forward-reverse", "reverse-forward"], "default": "reverse", "description": "Playback shape for the output video. reverse plays the clip backwards. forward-reverse makes a boomerang that plays forward then backward. reverse-forward starts with the reversed half, then returns forward." },
                    "audio":   { "type": "string", "enum": ["reverse", "keep", "mute"], "default": "reverse", "description": "How to handle sound: reverse matches the backwards picture, keep leaves a forward-playing copy of the original sound, and mute removes the audio track." },
                    "quality": { "type": "string", "enum": ["high", "balanced", "small"], "default": "balanced", "description": "H.264 output quality/size preset. high uses CRF 18, balanced uses CRF 23, and small uses CRF 28. Reversing always re-encodes because ffmpeg must reorder frames." }
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
    fn output_filename_uses_reversed_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mov", "-reversed", "mp4"),
            "clip-reversed.mp4"
        );
    }
}
