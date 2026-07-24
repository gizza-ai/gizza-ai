//! gizza-ai/seamless-loop-video — turn one video cycle into a genuinely
//! repeatable clip by rotating it at the midpoint and cross-dissolving the
//! source end back into its beginning.
//!
//! NOTE: ffmpeg cannot run in the chat Service Worker. The standalone page and
//! CLI are the supported execution surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_seamless_loop_video_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 50 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    duration: f64,
    #[serde(default = "default_crossfade")]
    crossfade: f64,
    #[serde(default = "default_audio")]
    audio: String,
    #[serde(default = "default_quality")]
    quality: String,
}

fn default_crossfade() -> f64 {
    1.0
}
fn default_audio() -> String {
    "remove".into()
}
fn default_quality() -> String {
    "balanced".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("duration")
                .required()
                .min(0.5)
                .max(600.0)
                .describe("Exact source clip duration in seconds, from 0.5 to 600. The tool rotates at half this value, so enter the real duration (for example 2 for a 2-second clip)."),
        )
        .param(
            Param::number("crossfade")
                .default(1.0)
                .min(0.05)
                .max(10.0)
                .describe("Seconds used to blend the original end into the beginning, from 0.05 to 10 (default 1). Must be shorter than half the clip duration; try 5-15% of the duration."),
        )
        .param(
            Param::enumv("audio", ["remove", "crossfade"])
                .default("remove")
                .describe("Audio handling: remove (default, works for every video) or crossfade (rotates and blends an existing audio track too; errors if the source has no audio)."),
        )
        .param(
            Param::enumv("quality", ["high", "balanced", "small"])
                .default("balanced")
                .describe("H.264 output quality: high (CRF 18), balanced (CRF 23, default), or small (CRF 28). Higher quality makes a larger file."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SeamlessLoopVideo;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/seamless-loop-video",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Make a video loop seamlessly with an end-to-start crossfade",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Make one video cycle repeat seamlessly: rotate the clip at its midpoint, cross-dissolve the original end into the original beginning, and return an H.264 MP4 whose outer boundary is continuous. Provide the video as url or ref and its exact duration in seconds (0.5-600). crossfade is 0.05-10 seconds (default 1, and must be shorter than half the clip). audio is remove (default) or crossfade for a source with an audio track. quality is high, balanced (default), or small. Best for steady shots and repeating motion; large start/end differences can ghost during the dissolve. Runs on the standalone page and CLI; chat ffmpeg is unavailable.",
        parameters = schema_json()
    ),
)]
impl SeamlessLoopVideo {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(value) => GuestResult::respond(value),
            Err(error) => GuestResult::error(error.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("seamless-loop-video")?;
    let (bytes, mime, input_name) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let input_ext = gizza_ai_block_utils::mime_to_ext(&mime).unwrap_or("mp4");
    let ffmpeg_input = format!("in.{input_ext}");
    let (argv, ffmpeg_output) = plan(
        &ffmpeg_input,
        args.duration,
        args.crossfade,
        &args.audio,
        &args.quality,
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, ffmpeg_input, bytes, ffmpeg_output)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&input_name, "-seamless-loop", "mp4");
    let for_llm = format!(
        "made {input_name} seamlessly loopable with a {}s end-to-start crossfade; audio {}, quality {} ({output_size} bytes video/mp4)",
        args.crossfade, args.audio, args.quality
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
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "duration": { "type": "number", "minimum": 0.5, "maximum": 600, "description": "Exact source clip duration in seconds, from 0.5 to 600. The tool rotates at half this value, so enter the real duration (for example 2 for a 2-second clip)." },
                    "crossfade": { "type": "number", "minimum": 0.05, "maximum": 10, "default": 1.0, "description": "Seconds used to blend the original end into the beginning, from 0.05 to 10 (default 1). Must be shorter than half the clip duration; try 5-15% of the duration." },
                    "audio": { "type": "string", "enum": ["remove", "crossfade"], "default": "remove", "description": "Audio handling: remove (default, works for every video) or crossfade (rotates and blends an existing audio track too; errors if the source has no audio)." },
                    "quality": { "type": "string", "enum": ["high", "balanced", "small"], "default": "balanced", "description": "H.264 output quality: high (CRF 18), balanced (CRF 23, default), or small (CRF 28). Higher quality makes a larger file." }
                },
                "additionalProperties": false,
                "required": ["duration"],
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
    fn output_filename_is_mp4_with_clear_suffix() {
        assert_eq!(
            filename_with_suffix("ocean.webm", "-seamless-loop", "mp4"),
            "ocean-seamless-loop.mp4"
        );
    }
}
