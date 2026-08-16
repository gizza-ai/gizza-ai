//! gizza-ai/video-to-dnxhd — fetch any video URL or attachment ref, force-transcode
//! it to an Avid DNxHR editing intermediate, and return a MOV or MXF envelope.
//!
//! This is an editorial/interchange tool, not a delivery optimizer: the output is
//! always DNxHR-family video via ffmpeg's `dnxhd` encoder, with profile-pinned
//! pixel formats and optional QuickTime MOV or MXF OP1a wrapping.
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
const MAX_OUTPUT_BYTES: usize = 160 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    container: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    pixel_format: Option<String>,
    #[serde(default)]
    audio: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv(
                "profile",
                ["dnxhr_lb", "dnxhr_sq", "dnxhr_hq", "dnxhr_hqx", "dnxhr_444"],
            )
            .default("dnxhr_sq")
            .describe(
                "DNxHR quality tier. dnxhr_lb is smallest/offline, dnxhr_sq is the default \
                 standard-quality editorial intermediate, dnxhr_hq is higher-quality 8-bit 4:2:2, \
                 dnxhr_hqx is 10-bit 4:2:2 for grading, and dnxhr_444 is 10-bit 4:4:4 for VFX/keying.",
            ),
        )
        .param(
            Param::enumv("container", ["mov", "mxf"])
                .default("mov")
                .describe(
                    "Wrapper for the DNxHR essence. mov (default) creates a QuickTime file that \
                     opens broadly; mxf creates an OP1a MXF interchange file for broadcast/Avid-style workflows.",
                ),
        )
        .param(
            Param::enumv("resolution", ["source", "2160p", "1080p", "720p"])
                .default("source")
                .describe(
                    "Optional downscale cap. source (default) preserves dimensions; 2160p, 1080p, \
                     and 720p cap the output height without upscaling smaller sources.",
                ),
        )
        .param(
            Param::enumv("pixel_format", ["auto", "yuv422p", "yuv422p10le", "yuv444p10le"])
                .default("auto")
                .describe(
                    "Pixel format override. auto (default) chooses the only format allowed by the \
                     selected DNxHR profile; explicit values are accepted only when they match that profile.",
                ),
        )
        .param(
            Param::enumv("audio", ["pcm16", "pcm24", "copy", "none"])
                .default("pcm16")
                .describe(
                    "Audio handling. pcm16 (default) writes uncompressed 16-bit PCM, pcm24 writes \
                     24-bit PCM, copy stream-copies source audio when the wrapper accepts it, and none drops audio.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoToDnxhd;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-to-dnxhd",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Transcode video to Avid DNxHR MOV or MXF for editing.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Transcode a video into an Avid DNxHR editing intermediate in MOV or MXF. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). profile is dnxhr_lb|dnxhr_sq|dnxhr_hq|dnxhr_hqx|dnxhr_444 (default dnxhr_sq); container is mov|mxf (default mov); resolution optionally downscales without upscaling; pixel_format normally stays auto because DNxHR profiles dictate their pixel formats; audio is pcm16|pcm24|copy|none (default pcm16). Output is a large NLE-friendly intermediate, not a small web-delivery encode.",
        parameters = schema_json()
    ),
)]
impl VideoToDnxhd {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-to-dnxhd")?;
    let profile = args.profile.as_deref().unwrap_or("dnxhr_sq");
    let container = args.container.as_deref().unwrap_or("mov");
    let resolution = args.resolution.as_deref().unwrap_or("source");
    let pixel_format = args.pixel_format.as_deref().unwrap_or("auto");
    let audio = args.audio.as_deref().unwrap_or("pcm16");

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = gizza_ai_video_to_dnxhd_core::plan(
        profile,
        container,
        resolution,
        pixel_format,
        audio,
        &ffmpeg_in,
    )
    .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-dnxhd args: {e}")))?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;
    let output_size = output.len();
    let out_mime = gizza_ai_video_to_dnxhd_core::output_mime(container)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-dnxhd args: {e}")))?;
    let out_ext = if container == "mxf" { "mxf" } else { "mov" };
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!(
        "transcoded {in_filename} ({in_mime}) to Avid DNxHR {profile} {container}, resolution={resolution}, pixel_format={pixel_format}, audio={audio} ({output_size} bytes)"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "profile": { "type": "string", "enum": ["dnxhr_lb", "dnxhr_sq", "dnxhr_hq", "dnxhr_hqx", "dnxhr_444"], "default": "dnxhr_sq", "description": "DNxHR quality tier. dnxhr_lb is smallest/offline, dnxhr_sq is the default standard-quality editorial intermediate, dnxhr_hq is higher-quality 8-bit 4:2:2, dnxhr_hqx is 10-bit 4:2:2 for grading, and dnxhr_444 is 10-bit 4:4:4 for VFX/keying." },
                    "container": { "type": "string", "enum": ["mov", "mxf"], "default": "mov", "description": "Wrapper for the DNxHR essence. mov (default) creates a QuickTime file that opens broadly; mxf creates an OP1a MXF interchange file for broadcast/Avid-style workflows." },
                    "resolution": { "type": "string", "enum": ["source", "2160p", "1080p", "720p"], "default": "source", "description": "Optional downscale cap. source (default) preserves dimensions; 2160p, 1080p, and 720p cap the output height without upscaling smaller sources." },
                    "pixel_format": { "type": "string", "enum": ["auto", "yuv422p", "yuv422p10le", "yuv444p10le"], "default": "auto", "description": "Pixel format override. auto (default) chooses the only format allowed by the selected DNxHR profile; explicit values are accepted only when they match that profile." },
                    "audio": { "type": "string", "enum": ["pcm16", "pcm24", "copy", "none"], "default": "pcm16", "description": "Audio handling. pcm16 (default) writes uncompressed 16-bit PCM, pcm24 writes 24-bit PCM, copy stream-copies source audio when the wrapper accepts it, and none drops audio." }
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
    fn output_filename_swaps_extension_to_selected_container() {
        assert_eq!(replace_extension("clip.mp4", "mov"), "clip.mov");
        assert_eq!(replace_extension("Scene.WEBM", "mxf"), "Scene.mxf");
    }
}
