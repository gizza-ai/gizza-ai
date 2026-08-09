//! gizza-ai/video-grayscale — fetch a video URL or attachment ref and convert
//! it to black-and-white (or a toned monochrome) with a single ffmpeg
//! `colorchannelmixer` pass, then return a media envelope.
//!
//! `method` picks the luma weighting (BT.709 / BT.601 / average / a single R, G
//! or B channel), `tint` tones the resulting gray (sepia / warm / cool /
//! cyanotype), `intensity` blends between the original and the toned gray, and
//! `contrast` optionally appends an `eq=contrast=` stage. All the matrix math
//! lives in `core` so chat, CLI, and the page share one implementation.
//!
//! The picture is rewritten, so the video is re-encoded to H.264 at the chosen
//! quality tier; audio is stream-copied when the container is kept, re-encoded
//! to AAC when it must switch to MP4, or dropped when `keep_audio` is false.
//!
//! NOTE: chat ffmpeg is non-functional (the chat runtime is a Service Worker
//! where ffmpeg can't load), so the supported surfaces are the standalone page
//! and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_grayscale_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

fn full() -> f64 {
    100.0
}
fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Luma weighting; empty/omitted → core default (`bt709`).
    #[serde(default)]
    method: String,
    #[serde(default = "full")]
    intensity: f64,
    /// Monochrome tone; empty/omitted → core default (`none`).
    #[serde(default)]
    tint: String,
    #[serde(default = "one")]
    contrast: f64,
    /// Encode tier; empty/omitted → core default (`balanced`).
    #[serde(default)]
    quality: String,
    #[serde(default = "yes")]
    keep_audio: bool,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("method", ["bt709", "bt601", "average", "red", "green", "blue"])
                .default("bt709")
                .describe("How the R/G/B channels are weighted into gray: bt709 (HD/sRGB luma, the default), bt601 (older SD weighting, slightly greener), average (equal thirds, flat and high-key), or red/green/blue to use a single channel — the darkroom color-filter look (red darkens a blue sky, green lightens foliage)."),
        )
        .param(
            Param::number("intensity")
                .default(100.0)
                .min(0.0)
                .max(100.0)
                .describe("How far to desaturate, 0–100 percent. 100 (default) is fully black-and-white, 50 is a half-faded look, 0 leaves the original colors untouched. Example: 60."),
        )
        .param(
            Param::enumv("tint", ["none", "sepia", "warm", "cool", "cyanotype"])
                .default("none")
                .describe("Tone applied to the gray: none (neutral black-and-white, the default), sepia (brown), warm (amber), cool (blue-gray), or cyanotype (strong blue). Folded into the same channel-mixer pass, so it costs nothing extra."),
        )
        .param(
            Param::number("contrast")
                .default(1.0)
                .min(0.5)
                .max(2.0)
                .describe("Contrast multiplier applied after the grayscale pass, 0.5–2.0. 1 (default) keeps the original tonality; 1.4 gives a punchy high-contrast black-and-white; below 1 flattens it. Example: 1.4."),
        )
        .param(
            Param::enumv("quality", ["fast", "balanced", "best"])
                .default("balanced")
                .describe("Encode tier for the H.264 output: fast (CRF 28, smallest/quickest), balanced (CRF 23, the default), or best (CRF 20, largest/slowest and closest to the source)."),
        )
        .param(
            Param::boolean("keep_audio")
                .default(true)
                .describe("Keep the original audio track (default true). Set false to drop the audio entirely for a silent-film look."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
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

#[cfg(target_arch = "wasm32")]
struct VideoGrayscale;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-grayscale",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a video to black-and-white or a toned monochrome",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a video to black-and-white in one ffmpeg pass. method = bt709|bt601|average|red|green|blue picks the luma weighting, intensity 0-100 blends between the original colors (0) and full grayscale (100, default), tint = none|sepia|warm|cool|cyanotype tones the gray, contrast 0.5-2.0 adds punch, quality = fast|balanced|best sets the H.264 CRF, keep_audio=false drops the audio track. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoGrayscale {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-grayscale")?;

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in,
        &args.method,
        args.intensity,
        &args.tint,
        args.contrast,
        &args.quality,
        args.keep_audio,
    )
    .map_err(SkillError::InvalidArgs)?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-bw", out_ext);
    let for_llm = format!(
        "converted {in_filename} to grayscale (method={} intensity={} tint={} contrast={} quality={} audio={}) → {output_size} bytes {out_mime}",
        if args.method.is_empty() { "bt709" } else { &args.method },
        args.intensity,
        if args.tint.is_empty() { "none" } else { &args.tint },
        args.contrast,
        if args.quality.is_empty() { "balanced" } else { &args.quality },
        if args.keep_audio { "kept" } else { "dropped" },
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "method":     { "type": "string", "enum": ["bt709", "bt601", "average", "red", "green", "blue"], "default": "bt709", "description": "How the R/G/B channels are weighted into gray: bt709 (HD/sRGB luma, the default), bt601 (older SD weighting, slightly greener), average (equal thirds, flat and high-key), or red/green/blue to use a single channel — the darkroom color-filter look (red darkens a blue sky, green lightens foliage)." },
                    "intensity":  { "type": "number", "minimum": 0, "maximum": 100, "default": 100.0, "description": "How far to desaturate, 0–100 percent. 100 (default) is fully black-and-white, 50 is a half-faded look, 0 leaves the original colors untouched. Example: 60." },
                    "tint":       { "type": "string", "enum": ["none", "sepia", "warm", "cool", "cyanotype"], "default": "none", "description": "Tone applied to the gray: none (neutral black-and-white, the default), sepia (brown), warm (amber), cool (blue-gray), or cyanotype (strong blue). Folded into the same channel-mixer pass, so it costs nothing extra." },
                    "contrast":   { "type": "number", "minimum": 0.5, "maximum": 2, "default": 1.0, "description": "Contrast multiplier applied after the grayscale pass, 0.5–2.0. 1 (default) keeps the original tonality; 1.4 gives a punchy high-contrast black-and-white; below 1 flattens it. Example: 1.4." },
                    "quality":    { "type": "string", "enum": ["fast", "balanced", "best"], "default": "balanced", "description": "Encode tier for the H.264 output: fast (CRF 28, smallest/quickest), balanced (CRF 23, the default), or best (CRF 20, largest/slowest and closest to the source)." },
                    "keep_audio": { "type": "boolean", "default": true, "description": "Keep the original audio track (default true). Set false to drop the audio entirely for a silent-film look." }
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
    fn omitted_params_deserialize_to_the_documented_defaults() {
        let args: Args = serde_json::from_str(r#"{"url":"https://example.com/clip.mp4"}"#).unwrap();
        assert_eq!(args.method, "");
        assert_eq!(args.intensity, 100.0);
        assert_eq!(args.tint, "");
        assert_eq!(args.contrast, 1.0);
        assert_eq!(args.quality, "");
        assert!(args.keep_audio);
        // The empty enum strings resolve to the same plan the named defaults do.
        let (from_empty, _) = plan("in.mp4", &args.method, args.intensity, &args.tint, args.contrast, &args.quality, args.keep_audio).unwrap();
        let (from_named, _) = plan("in.mp4", "bt709", 100.0, "none", 1.0, "balanced", true).unwrap();
        assert_eq!(from_empty, from_named);
    }

    #[test]
    fn output_filename_uses_bw_suffix() {
        assert_eq!(filename_with_suffix("clip.mov", "-bw", "mov"), "clip-bw.mov");
    }
}
