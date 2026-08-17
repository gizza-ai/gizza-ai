//! gizza-ai/mp4-to-flv — fetch any video URL or attachment ref, re-encode it to
//! an FLV (Flash Video) stream, and return an FLV envelope.
//!
//! FLV is the container legacy RTMP ingest endpoints and Flash-era players
//! expect. It accepts only a narrow codec set (H.264 or Sorenson Spark video;
//! AAC, MP3, Nellymoser, ADPCM or PCM audio), so this tool always RE-ENCODES —
//! an HEVC/AV1/Opus MP4 cannot be stream-copied into FLV at all. Use
//! `video-transcode` for modern mp4/webm delivery; use this when something
//! downstream still speaks RTMP/FLV.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const OUT_MIME: &str = "video/x-flv";
const OUT_EXT: &str = "flv";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    video_codec: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    fps: Option<String>,
    #[serde(default)]
    video_bitrate: Option<f64>,
    #[serde(default)]
    keyframe_seconds: Option<f64>,
    #[serde(default)]
    audio_codec: Option<String>,
    #[serde(default)]
    audio_bitrate: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("video_codec", ["h264", "flv1"])
                .default("h264")
                .describe(
                    "Video codec inside the FLV. h264 (default) is libx264, what modern RTMP \
                     ingest and Flash Player 9.0.115+ expect. flv1 is Sorenson Spark (H.263), \
                     the original Flash codec for pre-9 players and fixed-function decoders — \
                     much weaker compression, so raise video_bitrate when you pick it.",
                ),
        )
        .param(
            Param::enumv(
                "resolution",
                ["source", "1080p", "720p", "576p", "480p", "360p", "240p"],
            )
            .default("source")
            .describe(
                "Optional downscale cap. source (default) keeps the input dimensions; 1080p, \
                 720p, 576p, 480p, 360p, and 240p cap the output height without upscaling \
                 smaller sources. Width follows the source aspect ratio, and both axes are \
                 rounded to even numbers because H.264 rejects odd dimensions.",
            ),
        )
        .param(
            Param::enumv("fps", ["source", "60fps", "30fps", "25fps", "24fps", "15fps"])
                .default("source")
                .describe(
                    "Output frame rate. source (default) keeps the input timing; 60fps, 30fps, \
                     25fps (PAL), 24fps, and 15fps pin a constant rate, which some legacy RTMP \
                     ingest endpoints require. Lowering fps is the cheapest way to fit a low \
                     video_bitrate.",
                ),
        )
        .param(
            Param::integer("video_bitrate")
                .default(2500)
                .min(100.0)
                .max(20000.0)
                .describe(
                    "Target video bitrate in kbps (100-20000, default 2500). Applied as -b:v \
                     with a matching -maxrate and a 2-second -bufsize, so the stream stays \
                     inside the cap an RTMP endpoint allows. Rough guide: 800-1200 for 480p, \
                     2000-3000 for 720p, 4000-6000 for 1080p; add ~50% for flv1.",
                ),
        )
        .param(
            Param::integer("keyframe_seconds")
                .default(2)
                .min(1.0)
                .max(10.0)
                .describe(
                    "Seconds between forced keyframes (1-10, default 2). Two seconds is what \
                     most RTMP ingest endpoints ask for; it bounds how long a joining player \
                     waits for video. The interval is expressed in time, so it holds whatever \
                     frame rate the output ends up at.",
                ),
        )
        .param(
            Param::enumv("audio_codec", ["aac", "mp3", "none"])
                .default("aac")
                .describe(
                    "Audio codec inside the FLV. aac (default) is the modern RTMP choice and \
                     keeps the source sample rate. mp3 is the classic Flash codec and is \
                     resampled to 44100 Hz because FLV only permits MP3 at 44100/22050/11025 Hz. \
                     none drops audio entirely (-an) for a video-only stream.",
                ),
        )
        .param(
            Param::integer("audio_bitrate")
                .default(128)
                .min(32.0)
                .max(320.0)
                .describe(
                    "Target audio bitrate in kbps (32-320, default 128). Ignored when \
                     audio_codec is none. 96-128 is ample for speech, 160-192 for music.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Mp4ToFlv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mp4-to-flv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode a video to an FLV stream for legacy RTMP ingest.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Encode an MP4 (or any video ffmpeg can decode) into an FLV stream for legacy RTMP/Flash ingest pipelines — Flash Media Server, Wowza, Red5, and old Flash players. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). video_codec is h264|flv1 (default h264); resolution optionally caps the height without upscaling; fps optionally pins a constant frame rate; video_bitrate (kbps, default 2500) sets -b:v with a matching cap; keyframe_seconds (default 2) forces the RTMP-friendly keyframe interval; audio_codec is aac|mp3|none (default aac) with audio_bitrate in kbps. This ALWAYS re-encodes: FLV accepts only H.264/Sorenson video and AAC/MP3 audio, so HEVC, AV1, VP9, Opus, and FLAC sources cannot be stream-copied. Output is a .flv file.",
        parameters = schema_json()
    ),
)]
impl Mp4ToFlv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("mp4-to-flv")?;
    let video_codec = args.video_codec.as_deref().unwrap_or("h264");
    let resolution = args.resolution.as_deref().unwrap_or("source");
    let fps = args.fps.as_deref().unwrap_or("source");
    let video_bitrate = args
        .video_bitrate
        .unwrap_or(gizza_ai_mp4_to_flv_core::DEFAULT_VIDEO_BITRATE_KBPS as f64);
    let keyframe_seconds = args
        .keyframe_seconds
        .unwrap_or(gizza_ai_mp4_to_flv_core::DEFAULT_KEYFRAME_SECONDS as f64);
    let audio_codec = args.audio_codec.as_deref().unwrap_or("aac");
    let audio_bitrate = args
        .audio_bitrate
        .unwrap_or(gizza_ai_mp4_to_flv_core::DEFAULT_AUDIO_BITRATE_KBPS as f64);

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = gizza_ai_mp4_to_flv_core::plan(
        video_codec,
        resolution,
        fps,
        video_bitrate,
        keyframe_seconds,
        audio_codec,
        audio_bitrate,
        &ffmpeg_in,
    )
    .map_err(|e| SkillError::InvalidArgs(format!("invalid mp4-to-flv args: {e}")))?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;
    let output_size = output.len();
    let filename = replace_extension(&in_filename, OUT_EXT);
    let for_llm = format!(
        "encoded {in_filename} ({in_mime}) to an FLV stream: video_codec={video_codec}, \
         resolution={resolution}, fps={fps}, video_bitrate={video_bitrate}k, \
         keyframe_seconds={keyframe_seconds}, audio_codec={audio_codec} ({output_size} bytes)"
    );
    build_media_envelope(&output, OUT_MIME, filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "video_codec": { "type": "string", "enum": ["h264", "flv1"], "default": "h264", "description": "Video codec inside the FLV. h264 (default) is libx264, what modern RTMP ingest and Flash Player 9.0.115+ expect. flv1 is Sorenson Spark (H.263), the original Flash codec for pre-9 players and fixed-function decoders — much weaker compression, so raise video_bitrate when you pick it." },
                    "resolution": { "type": "string", "enum": ["source", "1080p", "720p", "576p", "480p", "360p", "240p"], "default": "source", "description": "Optional downscale cap. source (default) keeps the input dimensions; 1080p, 720p, 576p, 480p, 360p, and 240p cap the output height without upscaling smaller sources. Width follows the source aspect ratio, and both axes are rounded to even numbers because H.264 rejects odd dimensions." },
                    "fps": { "type": "string", "enum": ["source", "60fps", "30fps", "25fps", "24fps", "15fps"], "default": "source", "description": "Output frame rate. source (default) keeps the input timing; 60fps, 30fps, 25fps (PAL), 24fps, and 15fps pin a constant rate, which some legacy RTMP ingest endpoints require. Lowering fps is the cheapest way to fit a low video_bitrate." },
                    "video_bitrate": { "type": "integer", "default": 2500, "minimum": 100, "maximum": 20000, "description": "Target video bitrate in kbps (100-20000, default 2500). Applied as -b:v with a matching -maxrate and a 2-second -bufsize, so the stream stays inside the cap an RTMP endpoint allows. Rough guide: 800-1200 for 480p, 2000-3000 for 720p, 4000-6000 for 1080p; add ~50% for flv1." },
                    "keyframe_seconds": { "type": "integer", "default": 2, "minimum": 1, "maximum": 10, "description": "Seconds between forced keyframes (1-10, default 2). Two seconds is what most RTMP ingest endpoints ask for; it bounds how long a joining player waits for video. The interval is expressed in time, so it holds whatever frame rate the output ends up at." },
                    "audio_codec": { "type": "string", "enum": ["aac", "mp3", "none"], "default": "aac", "description": "Audio codec inside the FLV. aac (default) is the modern RTMP choice and keeps the source sample rate. mp3 is the classic Flash codec and is resampled to 44100 Hz because FLV only permits MP3 at 44100/22050/11025 Hz. none drops audio entirely (-an) for a video-only stream." },
                    "audio_bitrate": { "type": "integer", "default": 128, "minimum": 32, "maximum": 320, "description": "Target audio bitrate in kbps (32-320, default 128). Ignored when audio_codec is none. 96-128 is ample for speech, 160-192 for music." }
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
    fn output_filename_swaps_extension_to_flv() {
        assert_eq!(replace_extension("clip.mp4", "flv"), "clip.flv");
        assert_eq!(replace_extension("Stream.MOV", "flv"), "Stream.flv");
    }
}
