//! gizza-ai/waveform-image — fetch an audio URL or attachment ref and render a
//! static waveform PNG via ffmpeg's `showwavespic` filter. Mixed media shape:
//! audio in, image out (the envelope mime is always `image/png`).
//!
//! The chat schema is derived from `descriptor()` (single source — shared
//! shape across chat + CLI + page); the handler delegates source-resolution,
//! ffmpeg dispatch, and envelope-building to `block_utils`. Validation and the
//! pure argv builder live in `core`, shared with the page.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` remain native-compilable so the drift-guard + unit tests
// below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_waveform_image_core::plan_waveform_image;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
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
    split_channels: Option<bool>,
    #[serde(default)]
    scale: Option<String>,
    #[serde(default)]
    sampling: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::integer("width")
                .min(16.0)
                .max(4096.0)
                .default(1200)
                .describe("Output image width in pixels, 16-4096. Default 1200 (the common social-banner shape with the default height)."),
        )
        .param(
            Param::integer("height")
                .min(16.0)
                .max(2048.0)
                .default(300)
                .describe("Output image height in pixels, 16-2048. Default 300. With split_channels the height is divided across the channel lanes."),
        )
        .param(
            Param::string("color")
                .default("#4f46e5")
                .describe("Waveform color as hex: #RGB, #RRGGBB or #RRGGBBAA (alpha), e.g. #4f46e5 or #f00. With split_channels, a comma-separated list colors each channel (e.g. #ff0000,#0000ff). Default #4f46e5."),
        )
        .param(
            Param::string("color2")
                .describe("Optional gradient end color (same hex forms). When set, the wave is filled with a horizontal left-to-right gradient from color to color2; color must then be a single value. Empty = solid color."),
        )
        .param(
            Param::string("background")
                .describe("Background color as hex: #RGB, #RRGGBB or #RRGGBBAA — e.g. #000000, or #00000080 for a see-through scrim. Empty or omitted keeps the background transparent (the PNG has an alpha channel)."),
        )
        .param(
            Param::boolean("split_channels")
                .default(false)
                .describe("Draw each audio channel in its own horizontal lane instead of downmixing to one mono wave. Default false."),
        )
        .param(
            Param::enumv("scale", ["lin", "sqrt", "cbrt", "log"])
                .default("lin")
                .describe("Amplitude scale. lin is the true waveform; sqrt, cbrt and log progressively boost quiet audio so it stays visible. Default lin."),
        )
        .param(
            Param::enumv("sampling", ["average", "peak"])
                .default("average")
                .describe("Per-pixel-column sampling. average (default) draws the mean amplitude; peak draws the loudest sample — a fuller wave that keeps short hits visible. Default average."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WaveformImage;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/waveform-image",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a static waveform PNG image from an audio file",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Render a static waveform PNG image from an audio file (any format ffmpeg decodes: mp3, wav, flac, ogg, m4a, opus, ...). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). width x height set the image size in pixels (default 1200x300); color is the wave's hex color (default #4f46e5; alpha via #RRGGBBAA; a comma-separated list colors split channels); color2 turns the wave into a horizontal color->color2 gradient; background is an optional hex (empty = transparent PNG); split_channels draws one lane per channel instead of a single mono wave; scale (lin|sqrt|cbrt|log) boosts quiet audio's visibility; sampling=peak draws the loudest sample per column for a fuller wave. Output is always a PNG image.",
        parameters = schema_json()
    ),
)]
impl WaveformImage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; dimension/color/scale validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("waveform-image")?;
    let width = args.width.unwrap_or(0.0); // 0 = default (1200)
    let height = args.height.unwrap_or(0.0); // 0 = default (300)
    let color = args.color.as_deref().unwrap_or("");
    let color2 = args.color2.as_deref().unwrap_or("");
    let background = args.background.as_deref().unwrap_or("");
    let split_channels = args.split_channels.unwrap_or(false);
    let scale = args.scale.as_deref().unwrap_or("");
    let sampling = args.sampling.as_deref().unwrap_or("");

    // 2. Resolve source — URL fetch or attachment lookup (audio/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates size/colors/scale).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan_waveform_image(
        &ffmpeg_in, width, height, color, color2, background, split_channels, scale, sampling,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    // 5. Envelope — the output is always a PNG image (audio in, image out).
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-waveform", "png");
    let for_llm = format!(
        "rendered waveform image of {in_filename} ({output_size} bytes png)"
    );
    build_media_envelope(&output, "image/png", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. The `url`/`ref`
    /// property descriptions are centralized in `to_schema_json` (Audio
    /// wording), so the expected JSON uses that shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":            { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":            { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "width":          { "type": "integer", "minimum": 16, "maximum": 4096, "default": 1200, "description": "Output image width in pixels, 16-4096. Default 1200 (the common social-banner shape with the default height)." },
                    "height":         { "type": "integer", "minimum": 16, "maximum": 2048, "default": 300, "description": "Output image height in pixels, 16-2048. Default 300. With split_channels the height is divided across the channel lanes." },
                    "color":          { "type": "string", "default": "#4f46e5", "description": "Waveform color as hex: #RGB, #RRGGBB or #RRGGBBAA (alpha), e.g. #4f46e5 or #f00. With split_channels, a comma-separated list colors each channel (e.g. #ff0000,#0000ff). Default #4f46e5." },
                    "color2":         { "type": "string", "description": "Optional gradient end color (same hex forms). When set, the wave is filled with a horizontal left-to-right gradient from color to color2; color must then be a single value. Empty = solid color." },
                    "background":     { "type": "string", "description": "Background color as hex: #RGB, #RRGGBB or #RRGGBBAA — e.g. #000000, or #00000080 for a see-through scrim. Empty or omitted keeps the background transparent (the PNG has an alpha channel)." },
                    "split_channels": { "type": "boolean", "default": false, "description": "Draw each audio channel in its own horizontal lane instead of downmixing to one mono wave. Default false." },
                    "scale":          { "type": "string", "enum": ["lin", "sqrt", "cbrt", "log"], "default": "lin", "description": "Amplitude scale. lin is the true waveform; sqrt, cbrt and log progressively boost quiet audio so it stays visible. Default lin." },
                    "sampling":       { "type": "string", "enum": ["average", "peak"], "default": "average", "description": "Per-pixel-column sampling. average (default) draws the mean amplitude; peak draws the loudest sample — a fuller wave that keeps short hits visible. Default average." }
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
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_waveform_suffix_and_png_ext() {
        assert_eq!(
            filename_with_suffix("song.mp3", "-waveform", "png"),
            "song-waveform.png"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-waveform", "png"),
            "voice memo-waveform.png"
        );
    }
}
