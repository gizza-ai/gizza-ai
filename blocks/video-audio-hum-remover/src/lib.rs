//! gizza-ai/video-audio-hum-remover — fetch a video URL or attachment ref,
//! remove 50/60 Hz mains hum and its harmonics from its audio track via a chain
//! of ffmpeg band-reject (notch) filters, and return an envelope. The picture is
//! stream-copied (lossless); only the audio is re-encoded (the notch chain
//! rewrites samples). The chat schema is derived from `descriptor()` (single
//! source — shared across chat + CLI + page); source-resolution, ffmpeg
//! dispatch, and envelope-building are delegated to `block_utils`. The pure argv
//! builder and validation live in `core`.
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
use gizza_ai_video_audio_hum_remover_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    frequency: Option<String>,
    #[serde(default)]
    harmonics: Option<i64>,
    #[serde(default)]
    q: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("frequency", ["50", "60"])
                .default("50")
                .describe("Mains hum fundamental in Hz: 50 (Europe, Asia, Africa, Australia) or 60 (North & most of South America). Default 50."),
        )
        .param(
            Param::integer("harmonics")
                .min(0.0)
                .max(12.0)
                .default(4)
                .describe("How many harmonics above the fundamental to also notch (hum leaks into 2×, 3×… the base). 4 notches 50/100/150/200/250 Hz. 0 = fundamental only. Default 4."),
        )
        .param(
            Param::number("q")
                .min(1.0)
                .max(100.0)
                .default(10.0)
                .describe("Notch narrowness (Q), 1–100. Higher = narrower notch = less damage to nearby audio; 2–10 suits most mains hum. Default 10."),
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
struct VideoAudioHumRemover;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-hum-remover",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove 50/60 Hz mains hum from a video's audio",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Remove 50/60 Hz mains ('electrical') hum and its harmonics from a video's audio track with a tuned chain of narrow notch (band-reject) filters, keeping the picture untouched (the video stream is copied losslessly; only the audio is re-encoded). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). frequency is 50 (Europe/Asia/Africa) or 60 (Americas). harmonics (0–12, default 4) sets how many multiples above the fundamental are also notched. q (1–100, default 10) sets notch narrowness (higher = narrower = safer for nearby audio). The output keeps the input container. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoAudioHumRemover {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; frequency/harmonics/q validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-hum-remover")?;
    let frequency = args.frequency.as_deref().unwrap_or("50");
    let harmonics = args.harmonics.unwrap_or(4);
    let q = args.q.unwrap_or(10.0);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates frequency/harmonics/q).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) =
        plan(&ffmpeg_in, frequency, harmonics, q).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-dehummed", out_ext);
    let for_llm = format!(
        "hum removed from {in_filename} ({frequency} Hz + {harmonics} harmonics, q {q}) ({output_size} bytes {out_mime})"
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + frequency/harmonics/q), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "frequency": { "type": "string", "enum": ["50", "60"], "default": "50", "description": "Mains hum fundamental in Hz: 50 (Europe, Asia, Africa, Australia) or 60 (North & most of South America). Default 50." },
                    "harmonics": { "type": "integer", "minimum": 0, "maximum": 12, "default": 4, "description": "How many harmonics above the fundamental to also notch (hum leaks into 2×, 3×… the base). 4 notches 50/100/150/200/250 Hz. 0 = fundamental only. Default 4." },
                    "q":         { "type": "number", "minimum": 1, "maximum": 100, "default": 10.0, "description": "Notch narrowness (Q), 1–100. Higher = narrower notch = less damage to nearby audio; 2–10 suits most mains hum. Default 10." }
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
    fn output_filename_uses_dehummed_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-dehummed", "mp4"),
            "clip-dehummed.mp4"
        );
    }
}
