//! gizza-ai/video-fragmented-mp4 — convert progressive MP4s to fragmented MP4 via ffmpeg.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 50 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 60 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    fragment_duration: Option<f64>,
    #[serde(default)]
    keyframe_interval: Option<f64>,
    #[serde(default)]
    segment_index: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("mode", ["copy", "h264"])
                .default("copy")
                .describe("Conversion mode. copy (default) stream-copies every track into a fragmented MP4 with no quality loss. h264 re-encodes video to H.264/yuv420p and audio to AAC for browser compatibility, and can force a regular keyframe cadence."),
        )
        .param(
            Param::enumv("profile", ["mse", "dash", "cmaf"])
                .default("mse")
                .describe("Fragmented-MP4 profile. mse emits the generic Media Source Extensions flags. dash adds DASH-oriented muxer flags. cmaf adds CMAF-oriented muxer flags."),
        )
        .param(
            Param::number("fragment_duration")
                .default(0.0)
                .min(0.0)
                .max(30.0)
                .describe("Target minimum fragment duration in seconds, 0-30. 0 (default) cuts one fragment per existing keyframe. A positive value keeps fragments keyframe-aligned and prevents them being shorter than this target."),
        )
        .param(
            Param::number("keyframe_interval")
                .default(2.0)
                .min(0.0)
                .max(30.0)
                .describe("When mode=h264, force a keyframe every N seconds, 0-30. 2 seconds is the streaming-friendly default. Ignored in copy mode because existing encoded keyframes cannot be changed without re-encoding."),
        )
        .param(
            Param::boolean("segment_index")
                .default(false)
                .describe("Write a global sidx segment index near the start of the file. Useful for byte-range single-file DASH; off by default for the smallest generic MSE output."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-fragmented-mp4",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an MP4 into a fragmented MP4 for streaming playback",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a progressive MP4 (or another video ffmpeg can read) into a single-file fragmented MP4 for Media Source Extensions, DASH-style byte-range playback, or CMAF-oriented workflows. By default the tool stream-copies every track (`mode=copy`) and applies +frag_keyframe+empty_moov+default_base_moof so keys stay as media fragments without quality loss. `profile=dash` or `profile=cmaf` add the matching muxer flags, `fragment_duration` sets a minimum keyframe-aligned fragment length in seconds, `segment_index=true` writes a global sidx, and `mode=h264` re-encodes to H.264/AAC with a configurable keyframe interval for browser-compatible, evenly spaced fragments. Output is a single .mp4 file, not an HLS/DASH manifest or multi-file segment set. Provide the video as either url (HTTP/HTTPS) or ref. Note: runs on the standalone page and CLI; chat ffmpeg is unavailable.",
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
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-fragmented-mp4 args: {e}")))?;
    let (input_bytes, _in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let mode = args.mode.as_deref().unwrap_or("copy");
    let profile = args.profile.as_deref().unwrap_or("mse");
    let fragment_duration = args.fragment_duration.unwrap_or(0.0);
    let keyframe_interval = args
        .keyframe_interval
        .unwrap_or(gizza_ai_video_fragmented_mp4_core::DEFAULT_KEYFRAME_INTERVAL);
    let (argv, ffmpeg_out) = gizza_ai_video_fragmented_mp4_core::plan(
        mode,
        profile,
        fragment_duration,
        keyframe_interval,
        args.segment_index,
        "in.mp4",
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, "in.mp4".to_string(), input_bytes, ffmpeg_out)?;
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-fragmented", "mp4");
    let for_llm = format!(
        "converted {in_filename} to fragmented MP4 ({mode}/{profile}, {output_size} bytes, single-file fMP4)"
    );
    build_media_envelope(&output, "video/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + the five fragmenting knobs), so any
    /// future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["copy", "h264"], "default": "copy", "description": "Conversion mode. copy (default) stream-copies every track into a fragmented MP4 with no quality loss. h264 re-encodes video to H.264/yuv420p and audio to AAC for browser compatibility, and can force a regular keyframe cadence." },
                    "profile": { "type": "string", "enum": ["mse", "dash", "cmaf"], "default": "mse", "description": "Fragmented-MP4 profile. mse emits the generic Media Source Extensions flags. dash adds DASH-oriented muxer flags. cmaf adds CMAF-oriented muxer flags." },
                    "fragment_duration": { "type": "number", "minimum": 0, "maximum": 30, "default": 0.0, "description": "Target minimum fragment duration in seconds, 0-30. 0 (default) cuts one fragment per existing keyframe. A positive value keeps fragments keyframe-aligned and prevents them being shorter than this target." },
                    "keyframe_interval": { "type": "number", "minimum": 0, "maximum": 30, "default": 2.0, "description": "When mode=h264, force a keyframe every N seconds, 0-30. 2 seconds is the streaming-friendly default. Ignored in copy mode because existing encoded keyframes cannot be changed without re-encoding." },
                    "segment_index": { "type": "boolean", "default": false, "description": "Write a global sidx segment index near the start of the file. Useful for byte-range single-file DASH; off by default for the smallest generic MSE output." }
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
    fn output_filename_uses_fragmented_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mkv", "-fragmented", "mp4"),
            "clip-fragmented.mp4"
        );
    }
}
