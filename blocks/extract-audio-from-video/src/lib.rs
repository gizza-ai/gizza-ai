//! gizza-ai/extract-audio-from-video — fetch a video URL or attachment ref,
//! pull out the audio track via ffmpeg, return it as an MP3 or WAV envelope.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. The pure `core` argv builder
//! (format → audio codec) is shared with the standalone web page. Input is a
//! video, output is audio, so the page is `format="audio"`. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). The supporting imports +
// the Args type are only used inside that impl, so they look "unused" when
// running native unit tests. `descriptor()` / `schema_json()` stay
// native-compilable so the drift-guard below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_extract_audio_from_video_core::{build_argv, parse_format, DEFAULT_BITRATE};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    bitrate: Option<u32>,
}
fn default_format() -> String {
    "mp3".to_string()
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("format", ["mp3", "wav"])
                .default("mp3")
                .describe("Output audio format: mp3 (lossy) or wav (lossless). Default mp3."),
        )
        .param(
            Param::integer("bitrate")
                .min(32.0)
                .max(320.0)
                .describe("MP3 bitrate in kbps (32-320, default 192). Ignored for lossless wav."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractAudioFromVideo;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-audio-from-video",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the audio track from a video as MP3 or WAV",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Extract the audio track from a video file as MP3 (lossy) or WAV (lossless). Set format='mp3' (default) or 'wav', and optionally bitrate (kbps, 32-320, default 192, mp3 only). Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ExtractAudioFromVideo {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args (tool-specific — format enum + optional bitrate).
    let args: Args = serde_json::from_slice(&body).invalid_args("extract-audio-from-video")?;
    let fmt = parse_format(&args.format).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid extract-audio-from-video args: {e}"))
    })?;
    if let Some(b) = args.bitrate {
        if !(32..=320).contains(&b) {
            return Err(SkillError::InvalidArgs(
                "bitrate must be 32-320 kbps".to_string(),
            ));
        }
    }
    let bitrate = args.bitrate.unwrap_or(DEFAULT_BITRATE);

    // 2. Resolve source — URL fetch or attachment lookup (input is a video).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core). Output uses the chosen audio ext.
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{}", fmt.ext());
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, fmt, bitrate);

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — the extracted track is audio (mp3 or wav).
    let output_size = output.len();
    let filename = replace_extension(&in_filename, fmt.ext());
    let for_llm = format!(
        "extracted audio from {in_filename} as {} ({output_size} bytes: {filename})",
        fmt.mime()
    );
    build_media_envelope(
        output.as_slice(),
        fmt.mime(),
        filename,
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + format enum + optional bitrate).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format":  { "type": "string", "enum": ["mp3", "wav"], "default": "mp3", "description": "Output audio format: mp3 (lossy) or wav (lossless). Default mp3." },
                    "bitrate": { "type": "integer", "minimum": 32, "maximum": 320, "description": "MP3 bitrate in kbps (32-320, default 192). Ignored for lossless wav." }
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
    fn output_filename_swaps_to_audio_ext() {
        assert_eq!(replace_extension("clip.mp4", "mp3"), "clip.mp3");
        assert_eq!(replace_extension("movie.mkv", "wav"), "movie.wav");
    }
}
