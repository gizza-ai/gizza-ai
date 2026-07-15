//! gizza-ai/spritesheet-slice — fetch a sprite sheet (URL or attachment ref),
//! slice its grid of frames into individual PNGs, and return them bundled in a
//! ZIP.
//!
//! Pipeline: resolve the source image → `core::slice_spritesheet` (pure-Rust
//! `image` + `zip`) → base64 envelope (the ZIP as a downloadable file).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the ffmpeg media page shape — the same "no-page file-input"
//! pattern as extract-pdf-images / encrypt-file).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_spritesheet_slice_core::{OutFormat, SliceParams};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    columns: Option<u32>,
    rows: Option<u32>,
    tile_width: Option<u32>,
    tile_height: Option<u32>,
    #[serde(default)]
    margin: u32,
    #[serde(default)]
    spacing: u32,
    #[serde(default)]
    skip_empty: bool,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    max_frames: Option<usize>,
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf`; the grid is described either as
/// columns + rows or as tile_width + tile_height (validated in `core`).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("columns")
                .min(1.0)
                .describe("Number of columns in the grid. Pair with rows (grid mode)."),
        )
        .param(
            Param::integer("rows")
                .min(1.0)
                .describe("Number of rows in the grid. Pair with columns (grid mode)."),
        )
        .param(
            Param::integer("tile_width")
                .min(1.0)
                .describe("Frame width in pixels. Pair with tile_height (tile-size mode)."),
        )
        .param(
            Param::integer("tile_height")
                .min(1.0)
                .describe("Frame height in pixels. Pair with tile_width (tile-size mode)."),
        )
        .param(
            Param::integer("margin")
                .min(0.0)
                .default(0)
                .describe("Border in pixels around all four edges of the sheet."),
        )
        .param(
            Param::integer("spacing")
                .min(0.0)
                .default(0)
                .describe("Gap in pixels between adjacent frames."),
        )
        .param(
            Param::boolean("skip_empty")
                .default(false)
                .describe("Drop frames that are fully transparent (every pixel alpha 0)."),
        )
        .param(
            Param::enumv("format", ["png", "jpeg", "webp", "bmp"])
                .default("png")
                .describe("Per-frame image format. png/webp/bmp keep alpha; jpeg is opaque."),
        )
        .param(
            Param::string("prefix")
                .default("frame")
                .describe("Filename base for each frame, e.g. frame -> frame_000.png."),
        )
        .param(
            Param::integer("max_frames")
                .min(1.0)
                .describe("Stop after writing this many frames (default: all cells)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SpritesheetSlice;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spritesheet-slice",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Slice a grid sprite sheet into individual frame PNGs (ZIP)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Slice a grid sprite sheet into individual frames, returned bundled in a ZIP. Describe the grid either by columns + rows, or by tile_width + tile_height (fixed-size tiles); margin (outer border) and spacing (gap between frames) default to 0. Set skip_empty to drop fully-transparent frames, max_frames to cap how many are written, format to choose the per-frame encoding (png/jpeg/webp/bmp), and prefix to name them. Frames are named <prefix>_000.<ext>, <prefix>_001.<ext>, … (prefix defaults to frame) in row-major order. Provide the sheet as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl SpritesheetSlice {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("spritesheet-slice")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let format = OutFormat::parse(args.format.as_deref().unwrap_or("png"))
        .map_err(SkillError::InvalidArgs)?;
    let params = SliceParams {
        columns: args.columns,
        rows: args.rows,
        tile_width: args.tile_width,
        tile_height: args.tile_height,
        margin: args.margin,
        spacing: args.spacing,
        skip_empty: args.skip_empty,
        format,
        prefix: args.prefix.unwrap_or_else(|| "frame".to_string()),
        max_frames: args.max_frames,
    };

    let (zip, summary) = gizza_ai_spritesheet_slice_core::slice_spritesheet(&bytes, &params)
        .map_err(SkillError::InvalidArgs)?;

    let filename = replace_extension(&in_filename, "zip");
    let zip_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");

    let skipped_note = if summary.skipped_empty > 0 {
        format!(" ({} transparent frame(s) skipped)", summary.skipped_empty)
    } else {
        String::new()
    };
    let for_llm = format!(
        "sliced {in_filename} into {} frame(s) of {}x{} ({}x{} grid) → {filename} ({zip_len}-byte ZIP){skipped_note}",
        summary.frames, summary.tile_width, summary.tile_height, summary.columns, summary.rows
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/zip".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + grid params).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "columns": { "type": "integer", "minimum": 1, "description": "Number of columns in the grid. Pair with rows (grid mode)." },
                    "rows": { "type": "integer", "minimum": 1, "description": "Number of rows in the grid. Pair with columns (grid mode)." },
                    "tile_width": { "type": "integer", "minimum": 1, "description": "Frame width in pixels. Pair with tile_height (tile-size mode)." },
                    "tile_height": { "type": "integer", "minimum": 1, "description": "Frame height in pixels. Pair with tile_width (tile-size mode)." },
                    "margin": { "type": "integer", "minimum": 0, "default": 0, "description": "Border in pixels around all four edges of the sheet." },
                    "spacing": { "type": "integer", "minimum": 0, "default": 0, "description": "Gap in pixels between adjacent frames." },
                    "skip_empty": { "type": "boolean", "default": false, "description": "Drop frames that are fully transparent (every pixel alpha 0)." },
                    "format": { "type": "string", "enum": ["png", "jpeg", "webp", "bmp"], "default": "png", "description": "Per-frame image format. png/webp/bmp keep alpha; jpeg is opaque." },
                    "prefix": { "type": "string", "default": "frame", "description": "Filename base for each frame, e.g. frame -> frame_000.png." },
                    "max_frames": { "type": "integer", "minimum": 1, "description": "Stop after writing this many frames (default: all cells)." }
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
    fn output_filename_is_zip() {
        assert_eq!(replace_extension("hero.png", "zip"), "hero.zip");
    }
}
