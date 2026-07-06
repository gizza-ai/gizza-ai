//! gizza-ai/extract-frames — build a contact sheet (thumbnail grid) from a
//! video: sample frames at a fixed interval / fps / scene-change points and tile
//! them into one image, via ffmpeg.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI + page); the handler delegates source-resolution, ffmpeg
//! dispatch, and envelope-building to `block_utils`. Mode/value/grid/color
//! validation and the pure argv builder live in `core`, shared with the page.
//! The input is a video but the output is always a single image (the sheet), so
//! the page is `format="image"`. NOTE: chat ffmpeg is non-functional — the page
//! + CLI are the working surfaces.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_extract_frames_core::{format_ext, plan};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    columns: Option<u32>,
    #[serde(default)]
    rows: Option<u32>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

// Defaults — one source of truth for the handler fallbacks (the descriptor
// carries the same values for the chat/page-facing schema).
const DEFAULT_MODE: &str = "interval";
const DEFAULT_VALUE: f64 = 2.0;
const DEFAULT_COLUMNS: u32 = 4;
const DEFAULT_ROWS: u32 = 3;
const DEFAULT_WIDTH: u32 = 240;
const DEFAULT_BACKGROUND: &str = "white";
const DEFAULT_FORMAT: &str = "png";

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("mode", ["interval", "fps", "scene"])
                .default(DEFAULT_MODE)
                .describe("How frames are chosen: interval = one frame every `value` seconds; fps = `value` frames sampled per second; scene = frames at scene-change points (plus the opening frame). Default interval."),
        )
        .param(
            Param::number("value")
                .default(DEFAULT_VALUE)
                .describe("Rate/threshold for the chosen mode: seconds between frames for interval (e.g. 2), frames per second for fps (e.g. 1), or scene-change sensitivity 0-1 for scene (lower = more frames; typical 0.2-0.4). Default 2."),
        )
        .param(
            Param::integer("columns")
                .min(1.0)
                .max(8.0)
                .default(DEFAULT_COLUMNS)
                .describe("Grid columns — thumbnails per row (1-8). Default 4."),
        )
        .param(
            Param::integer("rows")
                .min(1.0)
                .max(8.0)
                .default(DEFAULT_ROWS)
                .describe("Grid rows (1-8). Up to columns x rows thumbnails fit on the sheet; the first grid's worth of sampled frames is used. Default 3."),
        )
        .param(
            Param::integer("width")
                .min(16.0)
                .max(800.0)
                .default(DEFAULT_WIDTH)
                .describe("Width of each thumbnail in pixels (16-800); height follows the source aspect ratio. Default 240."),
        )
        .param(
            Param::string("background")
                .default(DEFAULT_BACKGROUND)
                .describe("Grid gap/background color: a CSS color name (white, black, navy, ...) or hex like #1A2B3C or #f0a. Default white."),
        )
        .param(
            Param::enumv("format", ["png", "jpg"])
                .default(DEFAULT_FORMAT)
                .describe("Output image format: png (lossless, crisp) or jpg (smaller). Default png."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn out_mime(ext: &str) -> &'static str {
    match ext {
        "jpg" => "image/jpeg",
        _ => "image/png",
    }
}

#[cfg(target_arch = "wasm32")]
struct ExtractFrames;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-frames",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a contact sheet of video frames sampled by interval, fps or scene change",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Build a contact sheet (thumbnail grid) from a video: sample frames at a fixed interval (one frame every N seconds), a fixed fps (N frames per second), or at scene-change points, then tile them into a single columns x rows grid image. Use interval or fps to storyboard a clip evenly; use scene to capture cuts (scene mode always includes the opening frame, so even a cut-free clip yields a sheet). The sheet holds up to columns x rows thumbnails — the first grid's worth of sampled frames — so raise the interval or grid to cover a longer clip. Output is a single PNG (or JPG) image; a background color fills the grid gaps. Provide the video as either url (HTTP/HTTPS) or ref from a prior tool call. Note: chat ffmpeg is unavailable — runs on the standalone page and the CLI.",
        parameters = schema_json()
    ),
)]
impl ExtractFrames {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args; mode/value/grid/color validation lives in core's plan.
    let args: Args = serde_json::from_slice(&body).invalid_args("extract-frames")?;
    let mode = args.mode.as_deref().unwrap_or(DEFAULT_MODE);
    let value = args.value.unwrap_or(DEFAULT_VALUE);
    let columns = args.columns.unwrap_or(DEFAULT_COLUMNS);
    let rows = args.rows.unwrap_or(DEFAULT_ROWS);
    let width = args.width.unwrap_or(DEFAULT_WIDTH);
    let background = args.background.as_deref().unwrap_or(DEFAULT_BACKGROUND);
    let format = args.format.as_deref().unwrap_or(DEFAULT_FORMAT);

    // 2. Resolve source — URL fetch or attachment lookup (video/* MIME class).
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;

    // 3. Build ffmpeg argv (shared pure core — validates everything).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in, mode, value, columns, rows, width, background, format,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out.clone())?;

    // 5. Envelope — the sheet is a single image (PNG or JPEG).
    let out_ext = format_ext(format).map_err(SkillError::InvalidArgs)?;
    let mime = out_mime(out_ext);
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-contact-sheet", out_ext);
    let for_llm = format!(
        "contact sheet of {in_filename}: {columns}x{rows} grid, {mode} sampling (value {value}), {output_size} bytes {mime}"
    );
    build_media_envelope(&output, mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently. The `url`/`ref`
    /// property descriptions are centralized in `to_schema_json` (Video
    /// wording); integer bounds serialize as integers, the `number` default as a
    /// float.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":       { "type": "string", "enum": ["interval", "fps", "scene"], "default": "interval", "description": "How frames are chosen: interval = one frame every `value` seconds; fps = `value` frames sampled per second; scene = frames at scene-change points (plus the opening frame). Default interval." },
                    "value":      { "type": "number", "default": 2.0, "description": "Rate/threshold for the chosen mode: seconds between frames for interval (e.g. 2), frames per second for fps (e.g. 1), or scene-change sensitivity 0-1 for scene (lower = more frames; typical 0.2-0.4). Default 2." },
                    "columns":    { "type": "integer", "minimum": 1, "maximum": 8, "default": 4, "description": "Grid columns — thumbnails per row (1-8). Default 4." },
                    "rows":       { "type": "integer", "minimum": 1, "maximum": 8, "default": 3, "description": "Grid rows (1-8). Up to columns x rows thumbnails fit on the sheet; the first grid's worth of sampled frames is used. Default 3." },
                    "width":      { "type": "integer", "minimum": 16, "maximum": 800, "default": 240, "description": "Width of each thumbnail in pixels (16-800); height follows the source aspect ratio. Default 240." },
                    "background": { "type": "string", "default": "white", "description": "Grid gap/background color: a CSS color name (white, black, navy, ...) or hex like #1A2B3C or #f0a. Default white." },
                    "format":     { "type": "string", "enum": ["png", "jpg"], "default": "png", "description": "Output image format: png (lossless, crisp) or jpg (smaller). Default png." }
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
    fn output_filename_carries_contact_sheet_suffix() {
        assert_eq!(
            filename_with_suffix("holiday.mp4", "-contact-sheet", "png"),
            "holiday-contact-sheet.png"
        );
        assert_eq!(
            filename_with_suffix("clip.webm", "-contact-sheet", "jpg"),
            "clip-contact-sheet.jpg"
        );
    }
}
