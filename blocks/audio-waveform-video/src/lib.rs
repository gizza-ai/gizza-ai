//! gizza-ai/audio-waveform-video — fetch an audio URL or attachment ref and
//! render an animated waveform VIDEO (MP4, H.264 + AAC) via ffmpeg's `showwaves`
//! filter, with the original audio muxed back in so the wave stays in sync.
//! Mixed media shape: audio in, video out (the envelope mime is `video/mp4`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Validation and the pure
//! argv builder live in `core`, shared with the page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, format_to_mime_and_ext, resolve_source};
use gizza_ai_audio_waveform_video_core::plan_audio_waveform_video;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024; // video is heavier than the audio in

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    color2: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    scale: Option<String>,
    #[serde(default)]
    fps: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv("mode", ["mirror", "bars", "wave", "points"])
                .default("mirror")
                .describe(
                    "Waveform draw style. mirror (default) = a centered line, symmetric above and \
                     below the midline (the classic audiogram look); bars = one vertical bar per \
                     sample from the baseline; wave = a continuous line joining samples; points = a \
                     dot per sample.",
                ),
        )
        .param(
            Param::integer("width")
                .min(64.0)
                .max(3840.0)
                .default(1280)
                .describe(
                    "Output video width in pixels, 64-3840. Default 1280 (720p 16:9 with the \
                     default height). For a 9:16 vertical/story video use 720×1280; for a square \
                     post use 1080×1080.",
                ),
        )
        .param(
            Param::integer("height")
                .min(64.0)
                .max(2160.0)
                .default(720)
                .describe("Output video height in pixels, 64-2160. Default 720."),
        )
        .param(
            Param::string("color")
                .default("#4f46e5")
                .describe(
                    "Waveform color as hex: #RGB, #RRGGBB or #RRGGBBAA (alpha), e.g. #4f46e5 or \
                     #f00. Default #4f46e5.",
                ),
        )
        .param(
            Param::string("color2")
                .describe(
                    "Optional gradient end color (same hex forms). When set, the wave is filled \
                     with a horizontal left-to-right gradient from color to color2. Empty = solid \
                     color.",
                ),
        )
        .param(
            Param::string("background")
                .default("#101014")
                .describe(
                    "Background color as hex: #RGB, #RRGGBB or #RRGGBBAA — e.g. #000000. The video \
                     is opaque (MP4 has no transparency), so a solid backdrop is always drawn. \
                     Default #101014 (near-black slate).",
                ),
        )
        .param(
            Param::enumv("scale", ["lin", "sqrt", "cbrt", "log"])
                .default("lin")
                .describe(
                    "Amplitude scale. lin is the true waveform; sqrt, cbrt and log progressively \
                     boost quiet audio so the wave keeps moving. Default lin.",
                ),
        )
        .param(
            Param::integer("fps")
                .min(5.0)
                .max(60.0)
                .default(25)
                .describe(
                    "Video frame rate, 5-60. Higher = smoother motion and a larger file. Default \
                     25.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioWaveformVideo;

// The #[wafer_block] macro emits a native registration call requiring ::new() on
// the impl; skill-style impls don't have one. Gate the struct + impl to wasm32 so
// unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-waveform-video",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render an animated waveform video (MP4) from an audio file.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Render an animated waveform VIDEO (MP4, H.264 + AAC) from an audio file (any format ffmpeg decodes: mp3, wav, flac, ogg, m4a, opus, ...). The moving waveform is generated from the audio and the original audio is muxed back in, so it stays perfectly in sync — ready to post to social. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). mode is mirror|bars|wave|points (default mirror = the centered audiogram look); width x height set the frame size in pixels (default 1280x720; use 720x1280 for a 9:16 story or 1080x1080 for a square); color is the wave's hex color (default #4f46e5; alpha via #RRGGBBAA); color2 turns the wave into a horizontal color->color2 gradient; background is the hex backdrop (default #101014, always opaque); scale (lin|sqrt|cbrt|log) boosts quiet audio's motion; fps is the frame rate 5-60 (default 25). Output is always an MP4 video the length of the audio.",
        parameters = schema_json()
    ),
)]
impl AudioWaveformVideo {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; all validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("audio-waveform-video")?;
    let mode = args.mode.as_deref().unwrap_or("");
    let width = args.width.unwrap_or(0.0); // 0 = default (1280)
    let height = args.height.unwrap_or(0.0); // 0 = default (720)
    let color = args.color.as_deref().unwrap_or("");
    let color2 = args.color2.as_deref().unwrap_or("");
    let background = args.background.as_deref().unwrap_or("");
    let scale = args.scale.as_deref().unwrap_or("");
    let fps = args.fps.unwrap_or(0.0); // 0 = default (25)

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates size/colors/mode/scale).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (out_mime, out_ext) =
        format_to_mime_and_ext(AssetKind::Video, "mp4").expect("video/mp4 is a known format");
    let ffmpeg_out = format!("out.{out_ext}");
    let (argv, _out) = plan_audio_waveform_video(
        &ffmpeg_in, &ffmpeg_out, mode, width, height, color, color2, background, scale, fps,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — the output is always an MP4 video (audio in, video out).
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-waveform", "mp4");
    let for_llm = format!("rendered waveform video of {in_filename} ({output_size} bytes mp4)");
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. The `url`/`ref`
    /// property descriptions are centralized in `to_schema_json` (Audio wording).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":       { "type": "string", "enum": ["mirror", "bars", "wave", "points"], "default": "mirror", "description": "Waveform draw style. mirror (default) = a centered line, symmetric above and below the midline (the classic audiogram look); bars = one vertical bar per sample from the baseline; wave = a continuous line joining samples; points = a dot per sample." },
                    "width":      { "type": "integer", "minimum": 64, "maximum": 3840, "default": 1280, "description": "Output video width in pixels, 64-3840. Default 1280 (720p 16:9 with the default height). For a 9:16 vertical/story video use 720×1280; for a square post use 1080×1080." },
                    "height":     { "type": "integer", "minimum": 64, "maximum": 2160, "default": 720, "description": "Output video height in pixels, 64-2160. Default 720." },
                    "color":      { "type": "string", "default": "#4f46e5", "description": "Waveform color as hex: #RGB, #RRGGBB or #RRGGBBAA (alpha), e.g. #4f46e5 or #f00. Default #4f46e5." },
                    "color2":     { "type": "string", "description": "Optional gradient end color (same hex forms). When set, the wave is filled with a horizontal left-to-right gradient from color to color2. Empty = solid color." },
                    "background": { "type": "string", "default": "#101014", "description": "Background color as hex: #RGB, #RRGGBB or #RRGGBBAA — e.g. #000000. The video is opaque (MP4 has no transparency), so a solid backdrop is always drawn. Default #101014 (near-black slate)." },
                    "scale":      { "type": "string", "enum": ["lin", "sqrt", "cbrt", "log"], "default": "lin", "description": "Amplitude scale. lin is the true waveform; sqrt, cbrt and log progressively boost quiet audio so the wave keeps moving. Default lin." },
                    "fps":        { "type": "integer", "minimum": 5, "maximum": 60, "default": 25, "description": "Video frame rate, 5-60. Higher = smoother motion and a larger file. Default 25." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }
}
