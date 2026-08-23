//! gizza-ai/video-to-mxf — fetch any video URL or attachment ref and wrap it into
//! a SMPTE MXF file using a broadcast delivery profile (XDCAM HD422, XDCAM HD,
//! IMX 50 / D-10) or rewrap an already-compliant stream untouched.
//!
//! Container-first companion to the codec-first intermediates: `video-to-dnxhd`
//! owns DNxHR, `video-to-prores` owns ProRes; this one owns the MPEG-2-family
//! MXF delivery specs.
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
    resolution: Option<String>,
    #[serde(default)]
    frame_rate: Option<String>,
    #[serde(default)]
    audio: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("profile", ["xdcam_hd422", "xdcam_hd", "imx50", "copy"])
                .default("xdcam_hd422")
                .describe(
                    "Broadcast codec written into the MXF wrapper. xdcam_hd422 (default) is MPEG-2 \
                     4:2:2 at 50 Mbps CBR, the usual HD house standard; xdcam_hd is MPEG-2 4:2:0 at \
                     35 Mbps; imx50 is SMPTE D-10 / MPEG IMX 50 intra-only SD and requires \
                     frame_rate=25 with resolution=auto; copy rewraps the existing video stream \
                     without re-encoding it (audio still becomes PCM).",
                ),
        )
        .param(
            Param::enumv("resolution", ["auto", "source", "1920x1080", "1280x720"])
                .default("auto")
                .describe(
                    "Delivery raster. auto (default) conforms to the profile raster: 1920x1080 for \
                     the XDCAM profiles, 720x576 for imx50, source size for copy. source keeps the \
                     input size and encodes average-VBR instead of CBR (not spec-conformant). \
                     1920x1080 and 1280x720 conform explicitly. Sources are scaled to fit and \
                     padded with black, never cropped or stretched.",
                ),
        )
        .param(
            Param::enumv(
                "frame_rate",
                ["source", "23.976", "24", "25", "29.97", "30", "50", "59.94", "60"],
            )
            .default("source")
            .describe(
                "Output frame rate. source (default) keeps the input rate and relaxes the MXF \
                 edit-rate check so non-broadcast rates still wrap, at the cost of conformance. \
                 Pick 25 or 50 for 625/50 territories, 29.97 or 59.94 for 525/60, 23.976 or 24 for \
                 film. imx50 accepts only 25.",
            ),
        )
        .param(
            Param::enumv("audio", ["pcm16", "pcm24", "none"])
                .default("pcm16")
                .describe(
                    "Audio handling. MXF cannot carry AAC, so audio is always re-encoded: pcm16 \
                     (default) writes 48 kHz 16-bit PCM, pcm24 writes 48 kHz 24-bit PCM, and none \
                     drops audio for a picture-only delivery.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoToMxf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-to-mxf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Wrap video into a broadcast MXF file (XDCAM HD422, XDCAM HD, IMX 50).",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Wrap a video into a SMPTE MXF file for broadcast delivery. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). profile is xdcam_hd422|xdcam_hd|imx50|copy (default xdcam_hd422); resolution is auto|source|1920x1080|1280x720 (default auto, which conforms to the profile raster by scaling to fit and padding with black); frame_rate is source|23.976|24|25|29.97|30|50|59.94|60 (default source); audio is pcm16|pcm24|none (default pcm16, always 48 kHz PCM because MXF cannot carry AAC). imx50 writes SMPTE D-10 and requires frame_rate=25 with resolution=auto; copy rewraps the picture untouched so it rejects rescaling and retiming. Output is a large mezzanine/delivery file, not a web-delivery encode.",
        parameters = schema_json()
    ),
)]
impl VideoToMxf {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-to-mxf")?;
    let profile = args.profile.as_deref().unwrap_or("xdcam_hd422");
    let resolution = args.resolution.as_deref().unwrap_or("auto");
    let frame_rate = args.frame_rate.as_deref().unwrap_or("source");
    let audio = args.audio.as_deref().unwrap_or("pcm16");

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        gizza_ai_video_to_mxf_core::plan(profile, resolution, frame_rate, audio, &ffmpeg_in)
            .map_err(|e| SkillError::InvalidArgs(format!("invalid video-to-mxf args: {e}")))?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;
    let output_size = output.len();
    let filename = replace_extension(&in_filename, "mxf");
    let for_llm = format!(
        "wrapped {in_filename} ({in_mime}) into MXF profile={profile}, resolution={resolution}, frame_rate={frame_rate}, audio={audio} ({output_size} bytes)"
    );
    build_media_envelope(
        &output,
        gizza_ai_video_to_mxf_core::OUTPUT_MIME,
        filename,
        for_llm,
        MAX_OUTPUT_BYTES,
    )
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
                    "profile": { "type": "string", "enum": ["xdcam_hd422", "xdcam_hd", "imx50", "copy"], "default": "xdcam_hd422", "description": "Broadcast codec written into the MXF wrapper. xdcam_hd422 (default) is MPEG-2 4:2:2 at 50 Mbps CBR, the usual HD house standard; xdcam_hd is MPEG-2 4:2:0 at 35 Mbps; imx50 is SMPTE D-10 / MPEG IMX 50 intra-only SD and requires frame_rate=25 with resolution=auto; copy rewraps the existing video stream without re-encoding it (audio still becomes PCM)." },
                    "resolution": { "type": "string", "enum": ["auto", "source", "1920x1080", "1280x720"], "default": "auto", "description": "Delivery raster. auto (default) conforms to the profile raster: 1920x1080 for the XDCAM profiles, 720x576 for imx50, source size for copy. source keeps the input size and encodes average-VBR instead of CBR (not spec-conformant). 1920x1080 and 1280x720 conform explicitly. Sources are scaled to fit and padded with black, never cropped or stretched." },
                    "frame_rate": { "type": "string", "enum": ["source", "23.976", "24", "25", "29.97", "30", "50", "59.94", "60"], "default": "source", "description": "Output frame rate. source (default) keeps the input rate and relaxes the MXF edit-rate check so non-broadcast rates still wrap, at the cost of conformance. Pick 25 or 50 for 625/50 territories, 29.97 or 59.94 for 525/60, 23.976 or 24 for film. imx50 accepts only 25." },
                    "audio": { "type": "string", "enum": ["pcm16", "pcm24", "none"], "default": "pcm16", "description": "Audio handling. MXF cannot carry AAC, so audio is always re-encoded: pcm16 (default) writes 48 kHz 16-bit PCM, pcm24 writes 48 kHz 24-bit PCM, and none drops audio for a picture-only delivery." }
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
    fn output_filename_always_becomes_mxf() {
        assert_eq!(replace_extension("clip.mp4", "mxf"), "clip.mxf");
        assert_eq!(replace_extension("Scene.MOV", "mxf"), "Scene.mxf");
    }
}
