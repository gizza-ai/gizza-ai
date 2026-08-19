//! gizza-ai/video-fade — fetch a video URL or attachment ref, ramp the picture
//! up from (and down to) a solid colour at the start and end, ramp the sound out
//! of / into silence over the same spans, and return an envelope. The chat schema
//! is derived from `descriptor()` (single source — shared across chat + CLI +
//! page); source-resolution, ffmpeg dispatch, and envelope-building are delegated
//! to `block_utils`. Validation and the pure argv builder live in `core`.
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
use gizza_ai_video_fade_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

fn default_streams() -> String {
    "both".to_string()
}
fn default_color() -> String {
    "black".to_string()
}
fn default_quality() -> String {
    "balanced".to_string()
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    fade_in: f64,
    #[serde(default)]
    fade_out: f64,
    #[serde(default)]
    duration: f64,
    #[serde(default = "default_streams")]
    streams: String,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_quality")]
    quality: String,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("fade_in")
                .min(0.0)
                .max(30.0)
                .default(0.0)
                .describe("Length in seconds of the opening fade, ramped up from the fade colour (and from silence) at the START of the clip. 0 skips it. Range 0–30 s. At least one of fade_in / fade_out must be greater than 0."),
        )
        .param(
            Param::number("fade_out")
                .min(0.0)
                .max(30.0)
                .default(0.0)
                .describe("Length in seconds of the closing fade, ramped down to the fade colour (and to silence) at the END of the clip. 0 skips it. Range 0–30 s. Requires duration, because the fade-out is placed at duration − fade_out."),
        )
        .param(
            Param::number("duration")
                .min(0.0)
                .max(36000.0)
                .default(0.0)
                .describe("Exact length of the source clip in seconds (for example 12.5). Required whenever fade_out is greater than 0; leave at 0 for a fade-in-only run. fade_in + fade_out must not exceed it."),
        )
        .param(
            Param::enumv("streams", ["both", "video", "audio"])
                .default("both")
                .describe("Which streams to fade: both (default — picture and sound), video (picture only, the sound stays at full level) or audio (sound only; the picture is stream-copied untouched and the input container is kept)."),
        )
        .param(
            Param::string("color")
                .default("black")
                .describe("Colour the picture fades from and to: a name (black, white), a hex value (#101820 or 0x101820) or name@alpha (black@0.8). Default black. Ignored when streams=audio."),
        )
        .param(
            Param::enumv("quality", ["high", "balanced", "small"])
                .default("balanced")
                .describe("H.264 quality for the re-encoded picture: high (CRF 18), balanced (CRF 23, default) or small (CRF 28). Higher quality makes a larger file. Ignored when streams=audio, which copies the picture losslessly."),
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
struct VideoFade;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-fade",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fade a video in from and out to black at the start and end, sound included",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Add a fade-in at the start and/or a fade-out at the end of a video: the picture ramps up from (and down to) a solid colour — black by default — and the sound ramps out of and into silence over the same spans. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). fade_in and fade_out are lengths in seconds (0–30); 0 skips that side and at least one must be greater than 0. duration is the clip's exact length in seconds and is REQUIRED when fade_out is greater than 0 (the fade-out starts at duration − fade_out). streams picks both/video/audio, color sets the fade colour, quality picks the H.264 CRF. Fading the picture re-encodes to H.264/AAC MP4; streams=audio copies the picture untouched and keeps the input container. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoFade {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; every value is validated by core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-fade")?;

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in,
        args.fade_in,
        args.fade_out,
        args.duration,
        &args.streams,
        &args.color,
        &args.quality,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope with the output container's mime.
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-fade", out_ext);
    let for_llm = format!(
        "faded {in_filename} (fade-in {} s, fade-out {} s, streams {}, colour {}) into {output_size} bytes {out_mime}",
        args.fade_in, args.fade_out, args.streams, args.color
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + the fade params), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":      { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "fade_in":  { "type": "number", "minimum": 0, "maximum": 30, "default": 0.0, "description": "Length in seconds of the opening fade, ramped up from the fade colour (and from silence) at the START of the clip. 0 skips it. Range 0–30 s. At least one of fade_in / fade_out must be greater than 0." },
                    "fade_out": { "type": "number", "minimum": 0, "maximum": 30, "default": 0.0, "description": "Length in seconds of the closing fade, ramped down to the fade colour (and to silence) at the END of the clip. 0 skips it. Range 0–30 s. Requires duration, because the fade-out is placed at duration − fade_out." },
                    "duration": { "type": "number", "minimum": 0, "maximum": 36000, "default": 0.0, "description": "Exact length of the source clip in seconds (for example 12.5). Required whenever fade_out is greater than 0; leave at 0 for a fade-in-only run. fade_in + fade_out must not exceed it." },
                    "streams":  { "type": "string", "enum": ["both", "video", "audio"], "default": "both", "description": "Which streams to fade: both (default — picture and sound), video (picture only, the sound stays at full level) or audio (sound only; the picture is stream-copied untouched and the input container is kept)." },
                    "color":    { "type": "string", "default": "black", "description": "Colour the picture fades from and to: a name (black, white), a hex value (#101820 or 0x101820) or name@alpha (black@0.8). Default black. Ignored when streams=audio." },
                    "quality":  { "type": "string", "enum": ["high", "balanced", "small"], "default": "balanced", "description": "H.264 quality for the re-encoded picture: high (CRF 18), balanced (CRF 23, default) or small (CRF 28). Higher quality makes a larger file. Ignored when streams=audio, which copies the picture losslessly." }
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
    fn output_filename_uses_fade_suffix() {
        assert_eq!(
            filename_with_suffix("clip.webm", "-fade", "mp4"),
            "clip-fade.mp4"
        );
    }
}
