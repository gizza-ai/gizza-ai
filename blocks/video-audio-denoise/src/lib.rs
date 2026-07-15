//! gizza-ai/video-audio-denoise — fetch a video URL or attachment ref, reduce
//! background hiss/hum/noise in its audio track via ffmpeg's afftdn/anlmdn
//! denoiser, and return an envelope. The picture is stream-copied (lossless);
//! only the audio is re-encoded (the denoiser rewrites samples). The chat schema
//! is derived from `descriptor()` (single source — shared across chat + CLI +
//! page); source-resolution, ffmpeg dispatch, and envelope-building are
//! delegated to `block_utils`. Strength/method validation and the pure argv
//! builder live in `core`.
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
use gizza_ai_video_audio_denoise_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    strength: f64,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    remove_hum: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("strength")
                .required()
                .min(1.0)
                .max(100.0)
                .describe("How aggressively to reduce noise, 1–100 (higher removes more but risks a hollow/robotic sound). Start around 12 and raise gradually."),
        )
        .param(
            Param::enumv("method", ["afftdn", "anlmdn"])
                .default("afftdn")
                .describe("Denoiser: afftdn (FFT-based, fast, default) or anlmdn (non-local means, slower)."),
        )
        .param(
            Param::boolean("remove_hum")
                .default(false)
                .describe("Also cut low-frequency hum/rumble below 80 Hz with a high-pass filter. Default off."),
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
struct VideoAudioDenoise;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-denoise",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reduce background noise in a video's audio",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Reduce background hiss/hum/noise in a video's audio track, keeping the picture untouched (the video stream is copied losslessly; only the audio is re-encoded). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). strength is 1–100 (higher removes more but risks a hollow sound; start around 12). method is afftdn (FFT, default) or anlmdn (non-local means). Set remove_hum to also cut low-frequency hum below 80 Hz. The output keeps the input container. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioDenoise {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; strength/method validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-denoise")?;
    let method = args.method.as_deref().unwrap_or("afftdn");
    let remove_hum = args.remove_hum.unwrap_or(false);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates strength/method).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, args.strength, method, remove_hum).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-denoised", out_ext);
    let for_llm = format!(
        "denoised audio of {in_filename} ({method}, strength {}{}) ({output_size} bytes {out_mime})",
        args.strength,
        if remove_hum { ", hum removed" } else { "" }
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + strength/method/remove_hum), so any
    /// future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "strength":   { "type": "number", "minimum": 1, "maximum": 100, "description": "How aggressively to reduce noise, 1–100 (higher removes more but risks a hollow/robotic sound). Start around 12 and raise gradually." },
                    "method":     { "type": "string", "enum": ["afftdn", "anlmdn"], "default": "afftdn", "description": "Denoiser: afftdn (FFT-based, fast, default) or anlmdn (non-local means, slower)." },
                    "remove_hum": { "type": "boolean", "default": false, "description": "Also cut low-frequency hum/rumble below 80 Hz with a high-pass filter. Default off." }
                },
                "required": ["strength"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_denoised_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-denoised", "mp4"),
            "clip-denoised.mp4"
        );
    }
}
