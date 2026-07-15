//! gizza-ai/spritesheet-to-gif — fetch a grid sprite sheet (URL or attachment
//! ref), slice its frames in row-major order, and combine them into one animated
//! GIF.
//!
//! Pipeline: resolve the source image → `core::spritesheet_to_gif` (pure-Rust
//! `image` GIF encoder) → base64 envelope (the GIF as a downloadable file).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (an image-bytes output has no page render mode;
//! same "no-page media-bytes" pattern as gif-from-images / spritesheet-slice).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_spritesheet_to_gif_core::GifParams;
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
    delay_ms: Option<u16>,
    #[serde(default)]
    loop_count: u16,
    max_frames: Option<usize>,
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf`; the grid is described either as
/// columns + rows or as tile_width + tile_height (validated in `core`); the
/// animation is tuned with delay_ms + loop_count.
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
            Param::integer("delay_ms")
                .min(10.0)
                .max(60000.0)
                .default(100)
                .describe("Per-frame delay in milliseconds (10ms granularity)."),
        )
        .param(
            Param::integer("loop_count")
                .min(0.0)
                .default(0)
                .describe("Times to repeat: 0 loops forever, N plays N+1 times then stops."),
        )
        .param(
            Param::integer("max_frames")
                .min(1.0)
                .describe("Stop after this many frames (default: all cells)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SpritesheetToGif;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spritesheet-to-gif",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine a grid sprite sheet's frames into one animated GIF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Combine a grid sprite sheet's frames into a single animated GIF. Describe the grid either by columns + rows, or by tile_width + tile_height (fixed-size tiles); margin (outer border) and spacing (gap between frames) default to 0. Frames are taken in row-major (left-to-right, top-to-bottom) order. delay_ms sets the per-frame delay (default 100ms, 10ms granularity), loop_count controls repeats (0 = forever), skip_empty drops fully-transparent cells, and max_frames caps how many frames are used. Provide the sheet as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl SpritesheetToGif {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("spritesheet-to-gif")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let params = GifParams {
        columns: args.columns,
        rows: args.rows,
        tile_width: args.tile_width,
        tile_height: args.tile_height,
        margin: args.margin,
        spacing: args.spacing,
        skip_empty: args.skip_empty,
        delay_ms: args.delay_ms.unwrap_or(100),
        loop_count: args.loop_count,
        max_frames: args.max_frames,
    };

    let (gif, summary) = gizza_ai_spritesheet_to_gif_core::spritesheet_to_gif(&bytes, &params)
        .map_err(SkillError::InvalidArgs)?;

    let filename = replace_extension(&in_filename, "gif");
    let gif_len = gif.len();
    let encoded = B64.encode(&gif);
    let data_url = format!("data:image/gif;base64,{encoded}");

    let skipped_note = if summary.skipped_empty > 0 {
        format!(" ({} transparent frame(s) skipped)", summary.skipped_empty)
    } else {
        String::new()
    };
    let loop_note = if params.loop_count == 0 {
        "loops forever".to_string()
    } else {
        format!("plays {} time(s)", params.loop_count as u32 + 1)
    };
    let for_llm = format!(
        "combined {in_filename} ({}x{} grid) into a {}-frame animated GIF at {}ms/frame ({loop_note}) → {filename} ({gif_len}-byte GIF){skipped_note}",
        summary.columns, summary.rows, summary.frames, summary.delay_ms
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "image/gif".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + grid/animation params).
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
                    "delay_ms": { "type": "integer", "minimum": 10, "maximum": 60000, "default": 100, "description": "Per-frame delay in milliseconds (10ms granularity)." },
                    "loop_count": { "type": "integer", "minimum": 0, "default": 0, "description": "Times to repeat: 0 loops forever, N plays N+1 times then stops." },
                    "max_frames": { "type": "integer", "minimum": 1, "description": "Stop after this many frames (default: all cells)." }
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
    fn output_filename_is_gif() {
        assert_eq!(replace_extension("hero.png", "gif"), "hero.gif");
    }
}
