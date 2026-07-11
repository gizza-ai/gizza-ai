//! gizza-ai/video-caption-burner — hard-burn ("open") an SRT or WebVTT subtitle
//! track onto a video with ffmpeg.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Subtitle parsing, style
//! validation and the pure argv builder live in `core`, shared with the page.
//!
//! Each cue's text is written to ffmpeg's virtual FS as its own `textfile` and
//! the bundled font as a `fontfile` (both via [`dispatch_ffmpeg_inputs`]) — so
//! the filtergraph carries no user text (no escaping / injection) and the
//! browser ffmpeg, which has no system fonts, still renders. NOTE: chat ffmpeg
//! is non-functional (the chat runtime is a Service Worker where ffmpeg can't
//! load), so the supported surfaces are the standalone page and the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg_inputs call host imports → wasm-only.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_inputs, resolve_source};
use gizza_ai_video_caption_burner_core::{plan, FONT_BYTES, FONT_FILE};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

fn default_background() -> bool {
    true
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    subtitles: String,
    #[serde(default)]
    position: Option<String>,
    #[serde(default)]
    font_size: Option<u32>,
    #[serde(default)]
    font_color: Option<String>,
    #[serde(default = "default_background")]
    background: bool,
    #[serde(default)]
    background_color: Option<String>,
    #[serde(default)]
    background_opacity: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::string("subtitles")
                .required()
                .describe("The caption track to burn in, as SRT or WebVTT text (the whole file's contents, including the `HH:MM:SS,mmm --> HH:MM:SS,mmm` timing lines). Each timed cue is drawn during its own window; inline tags like <i> are stripped. Text is drawn literally, so no escaping is needed."),
        )
        .param(
            Param::enumv("position", ["bottom", "center", "top"])
                .default("bottom")
                .describe("Where to anchor the captions vertically: bottom, center or top (always horizontally centered). Default bottom."),
        )
        .param(
            Param::integer("font_size")
                .min(8.0)
                .max(200.0)
                .default(24)
                .describe("Caption font size in pixels (8-200). Default 24."),
        )
        .param(
            Param::string("font_color")
                .default("#ffffff")
                .describe("Caption text color: a CSS color name (white, black, yellow, ...) or hex like #FFCC00 / #fc0. Default #ffffff (white)."),
        )
        .param(
            Param::boolean("background")
                .default(true)
                .describe("Draw a semi-transparent box behind each caption for readability. Default true."),
        )
        .param(
            Param::string("background_color")
                .default("#000000")
                .describe("Background box color (CSS name or hex). Default #000000 (black). Ignored when background is false."),
        )
        .param(
            Param::number("background_opacity")
                .min(0.0)
                .max(1.0)
                .default(0.5)
                .describe("Background box opacity from 0 (transparent) to 1 (solid). Default 0.5. Ignored when background is false."),
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
struct VideoCaptionBurner;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-caption-burner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Burn SRT or WebVTT subtitles permanently into a video",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Hard-burn (\"open\") an SRT or WebVTT subtitle track onto a video so the captions are baked into the pixels — ideal for social clips that autoplay muted. Paste the subtitle file contents as `subtitles`; each timed cue is drawn only during its own start-end window. Optionally set position (bottom/center/top — default bottom), font size, text color, and a semi-transparent background box (color + opacity). Inline markup like <i>/<b> is stripped and text is drawn literally (no escaping needed) with a bundled bold sans-serif font. Provide the video as either url (HTTP/HTTPS) or ref (id from a prior tool call). Video re-encodes to H.264; audio is stream-copied when the container is kept (mp4/mov/m4v/mkv) or re-encoded to AAC when converting to mp4; MP4/MOV get +faststart. Note: runs on the standalone page and the CLI (chat ffmpeg is unavailable).",
        parameters = schema_json()
    ),
)]
impl VideoCaptionBurner {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-caption-burner")?;
    let position = args.position.as_deref().unwrap_or("bottom");
    let font_size = args.font_size.unwrap_or(24);
    let font_color = args.font_color.as_deref().unwrap_or("#ffffff");
    let background_color = args.background_color.as_deref().unwrap_or("#000000");
    let background_opacity = args.background_opacity.unwrap_or(0.5);

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out, cue_files) = plan(
        &ffmpeg_in,
        &args.subtitles,
        position,
        font_size,
        font_color,
        args.background,
        background_color,
        background_opacity,
    )
    .map_err(SkillError::InvalidArgs)?;

    // Write the media, the bundled font, and each cue's text into ffmpeg's FS.
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::with_capacity(cue_files.len() + 2);
    inputs.push((ffmpeg_in.clone(), input_bytes));
    inputs.push((FONT_FILE.to_string(), FONT_BYTES.to_vec()));
    for (name, text) in cue_files.iter() {
        inputs.push((name.clone(), text.clone().into_bytes()));
    }
    let output = dispatch_ffmpeg_inputs(argv, inputs, ffmpeg_out.clone())?;

    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-captioned", out_ext);
    let for_llm = format!(
        "burned {} subtitle cue(s) into {in_filename} ({output_size} bytes {out_mime})",
        cue_files.len()
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Video url⊕ref oneOf + the seven styling params), so any
    /// future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "subtitles": { "type": "string", "description": "The caption track to burn in, as SRT or WebVTT text (the whole file's contents, including the `HH:MM:SS,mmm --> HH:MM:SS,mmm` timing lines). Each timed cue is drawn during its own window; inline tags like <i> are stripped. Text is drawn literally, so no escaping is needed." },
                    "position": {
                        "type": "string",
                        "enum": ["bottom", "center", "top"],
                        "default": "bottom",
                        "description": "Where to anchor the captions vertically: bottom, center or top (always horizontally centered). Default bottom."
                    },
                    "font_size": { "type": "integer", "minimum": 8, "maximum": 200, "default": 24, "description": "Caption font size in pixels (8-200). Default 24." },
                    "font_color": { "type": "string", "default": "#ffffff", "description": "Caption text color: a CSS color name (white, black, yellow, ...) or hex like #FFCC00 / #fc0. Default #ffffff (white)." },
                    "background": { "type": "boolean", "default": true, "description": "Draw a semi-transparent box behind each caption for readability. Default true." },
                    "background_color": { "type": "string", "default": "#000000", "description": "Background box color (CSS name or hex). Default #000000 (black). Ignored when background is false." },
                    "background_opacity": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5, "description": "Background box opacity from 0 (transparent) to 1 (solid). Default 0.5. Ignored when background is false." }
                },
                "required": ["subtitles"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_captioned_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-captioned", "mp4"),
            "clip-captioned.mp4"
        );
    }
}
