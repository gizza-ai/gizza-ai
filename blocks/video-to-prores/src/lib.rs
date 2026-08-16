//! gizza-ai/video-to-prores — fetch any video URL or attachment ref, force-transcode
//! it to an Apple ProRes 422 QuickTime intermediate, and return a MOV envelope.
//!
//! This is an editorial/interchange tool, not a delivery optimizer: the output is
//! always ProRes 422-family video in a `.mov` container with 10-bit 4:2:2 pixels
//! and the Apple vendor atom. Use `video-to-h264` or `video-transcode` for small
//! web-delivery files; use this when an editor/NLE wants ProRes media.
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
const MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;
const OUT_MIME: &str = "video/quicktime";
const OUT_EXT: &str = "mov";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    audio: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("profile", ["proxy", "lt", "standard", "hq"])
                .default("standard")
                .describe(
                    "ProRes 422 tier. proxy is smallest/offline (~45 Mbps at 1080p29.97), \
                     lt is lighter editorial (~102 Mbps), standard is plain ProRes 422 and the \
                     default (~147 Mbps), hq is largest/headroom for grading (~220 Mbps). All \
                     choices are 10-bit 4:2:2 in a MOV container.",
                ),
        )
        .param(
            Param::enumv(
                "resolution",
                ["source", "2160p", "1440p", "1080p", "720p", "540p", "480p"],
            )
            .default("source")
            .describe(
                "Optional downscale cap. source (default) preserves the input dimensions; 2160p, \
                 1440p, 1080p, 720p, 540p, and 480p cap the output height without upscaling \
                 smaller sources. Downscaling is the practical size lever for huge ProRes files.",
            ),
        )
        .param(
            Param::enumv("audio", ["pcm16", "pcm24", "none"])
                .default("pcm16")
                .describe(
                    "Audio handling for the MOV. pcm16 (default) writes uncompressed 16-bit PCM, \
                     pcm24 writes uncompressed 24-bit PCM for 24-bit masters, and none drops audio \
                     with -an for a picture-only intermediate.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoToProres;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-to-prores",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Transcode video to Apple ProRes 422 MOV for editing.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Transcode a video into an Apple ProRes 422 QuickTime MOV intermediate for Final Cut Pro, Premiere Pro, DaVinci Resolve, Avid, and other editorial workflows. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). profile is proxy|lt|standard|hq (default standard); resolution optionally downscales without upscaling; audio is pcm16|pcm24|none (default pcm16). Output is always a large edit-friendly .mov with prores_ks, yuv422p10le, -vendor apl0, and uncompressed PCM unless audio=none. This is not a small web-delivery encode.",
        parameters = schema_json()
    ),
)]
impl VideoToProres {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-to-prores")?;
    let profile = args.profile.as_deref().unwrap_or("standard");
    let resolution = args.resolution.as_deref().unwrap_or("source");
    let audio = args.audio.as_deref().unwrap_or("pcm16");

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = gizza_ai_video_to_prores_core::plan(
        profile,
        resolution,
        audio,
        &ffmpeg_in,
    )
    .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-prores args: {e}")))?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;
    let output_size = output.len();
    let filename = replace_extension(&in_filename, OUT_EXT);
    let for_llm = format!(
        "transcoded {in_filename} ({in_mime}) to Apple ProRes 422 {profile} MOV, resolution={resolution}, audio={audio} ({output_size} bytes)"
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
                    "profile": { "type": "string", "enum": ["proxy", "lt", "standard", "hq"], "default": "standard", "description": "ProRes 422 tier. proxy is smallest/offline (~45 Mbps at 1080p29.97), lt is lighter editorial (~102 Mbps), standard is plain ProRes 422 and the default (~147 Mbps), hq is largest/headroom for grading (~220 Mbps). All choices are 10-bit 4:2:2 in a MOV container." },
                    "resolution": { "type": "string", "enum": ["source", "2160p", "1440p", "1080p", "720p", "540p", "480p"], "default": "source", "description": "Optional downscale cap. source (default) preserves the input dimensions; 2160p, 1440p, 1080p, 720p, 540p, and 480p cap the output height without upscaling smaller sources. Downscaling is the practical size lever for huge ProRes files." },
                    "audio": { "type": "string", "enum": ["pcm16", "pcm24", "none"], "default": "pcm16", "description": "Audio handling for the MOV. pcm16 (default) writes uncompressed 16-bit PCM, pcm24 writes uncompressed 24-bit PCM for 24-bit masters, and none drops audio with -an for a picture-only intermediate." }
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
    fn output_filename_swaps_extension_to_mov() {
        assert_eq!(replace_extension("clip.mp4", "mov"), "clip.mov");
        assert_eq!(replace_extension("Scene.WEBM", "mov"), "Scene.mov");
    }
}
