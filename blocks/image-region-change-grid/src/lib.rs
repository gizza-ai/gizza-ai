//! gizza-ai/image-region-change-grid — divide two aligned images into a grid
//! and report which cells changed and by how much.
//!
//! Pipeline: resolve both image sources (URL/ref) → pure `core::compare`
//! (decode + optional resize + per-pixel delta + per-cell aggregation via the
//! `image` crate) → JSON report. `Input::None` + a required `images`
//! source_list (like image-collage / duplicate-image-finder).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (the generated page form is a single file
//! upload, and this tool needs two images).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{resolve_source, respond_ok, AssetKind};
use gizza_ai_block_utils::{Input, Param, SkillError, SourceFields, ToolDescriptor};
use gizza_ai_image_region_change_grid_core::{parse_metric, parse_size_mismatch, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// Each image is capped at 8 MiB on the wire (matches image-collage /
/// duplicate-image-finder).
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_axis")]
    columns: u32,
    #[serde(default = "default_axis")]
    rows: u32,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_min_change")]
    min_change: f64,
    #[serde(default = "default_metric")]
    metric: String,
    #[serde(default = "default_size_mismatch")]
    size_mismatch: String,
    #[serde(default = "default_map")]
    map: bool,
}
fn default_axis() -> u32 {
    4
}
fn default_threshold() -> f64 {
    2.0
}
fn default_min_change() -> f64 {
    1.0
}
fn default_metric() -> String {
    "rgb".to_string()
}
fn default_size_mismatch() -> String {
    "resize".to_string()
}
fn default_map() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 2)
                .required()
                .describe("Exactly two image sources (PNG/JPEG/WebP/GIF/BMP), the before image first and the after image second. Each item has exactly one of `url` or `ref`. The first image defines the canvas the grid is measured on."),
        )
        .param(
            Param::integer("columns")
                .default(4)
                .min(1.0)
                .max(32.0)
                .describe("Number of grid columns, 1-32 (default 4). Columns are labelled A, B, C ... left to right, so a cell reads like `C2`. Cannot exceed the first image's width in pixels."),
        )
        .param(
            Param::integer("rows")
                .default(4)
                .min(1.0)
                .max(32.0)
                .describe("Number of grid rows, 1-32 (default 4). Rows are numbered 1 upward from the top. Cannot exceed the first image's height in pixels."),
        )
        .param(
            Param::number("threshold")
                .default(2.0)
                .min(0.0)
                .max(100.0)
                .describe("Per-pixel sensitivity, 0-100 percent of the maximum possible colour difference. A pixel counts as changed when its difference is strictly greater than this. The default 2 ignores JPEG re-compression noise; use 0 to count every non-identical pixel, or raise it to 10+ to catch only obvious changes."),
        )
        .param(
            Param::number("min_change")
                .default(1.0)
                .min(0.0)
                .max(100.0)
                .describe("Noise filter, 0-100 percent. A cell is flagged as changed only when at least this share of its pixels changed (default 1). Every cell's numbers are still reported; this only decides the `changed` flag, the changed-cell count and the ranked shortlist."),
        )
        .param(
            Param::enumv("metric", ["rgb", "luma", "max-channel"])
                .default("rgb")
                .describe("How each pixel's difference is measured. rgb (default) is the root-mean-square difference across red, green, blue and alpha. luma compares perceived brightness only (Rec. 601), so a recolour at the same brightness reads as unchanged. max-channel takes the single largest channel difference and is the strictest."),
        )
        .param(
            Param::enumv("size_mismatch", ["resize", "error"])
                .default("resize")
                .describe("What to do when the two images are different sizes. resize (default) scales the second image onto the first image's canvas before comparing; error refuses the comparison instead."),
        )
        .param(
            Param::boolean("map")
                .default(true)
                .describe("Include an ASCII density map of the grid (one line per row, one glyph per cell, plus a legend) alongside the numbers. On by default; set false for numbers only."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageRegionChangeGrid;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-region-change-grid",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report which grid cells changed between two aligned images",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Divide two aligned images into a grid and report which cells changed and by how much — a compact 'what changed where' summary. Provide `images` as exactly two sources (each a url or a `ref`; PNG/JPEG/WebP/GIF/BMP): the before image first, the after image second. The first image defines the canvas; if the second differs in size it is scaled onto that canvas (`size_mismatch=resize`, the default) or rejected (`error`). `columns` and `rows` set the grid (1-32 each, default 4x4); cells are labelled spreadsheet-style, column letter plus row number (C2). `threshold` is the per-pixel sensitivity in percent of the maximum colour difference (default 2, which ignores re-compression noise); `min_change` is the share of a cell's pixels that must change before the cell is flagged (default 1). `metric` picks how a pixel's difference is measured: rgb (default, RMS over red/green/blue/alpha), luma (perceived brightness only, so an equal-brightness recolour reads as unchanged), or max-channel (largest single-channel difference, strictest). The result gives overall changed-pixel count and percentage, mean and max delta, and for every cell its rectangle, changed-pixel count and percentage, mean/max delta and changed flag, plus a ranked shortlist of the most-changed cells, a one-line summary, and (unless `map` is false) an ASCII density map of the grid. Output is text/JSON, not a diff image.",
        parameters = schema_json()
    ),
)]
impl ImageRegionChangeGrid {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::SkillResultExt;

    let args: Args = serde_json::from_slice(&body).invalid_args("image-region-change-grid")?;
    if args.images.len() != 2 {
        return Err(SkillError::InvalidArgs(format!(
            "image-region-change-grid compares exactly 2 images (before, after); got {}",
            args.images.len()
        )));
    }

    let opts = Options {
        columns: args.columns,
        rows: args.rows,
        threshold: args.threshold,
        min_change: args.min_change,
        metric: parse_metric(&args.metric).map_err(SkillError::InvalidArgs)?,
        size_mismatch: parse_size_mismatch(&args.size_mismatch).map_err(SkillError::InvalidArgs)?,
        map: args.map,
    };

    let mut bytes: Vec<Vec<u8>> = Vec::with_capacity(2);
    for field in args.images.into_iter() {
        let (b, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        bytes.push(b);
    }

    let report = gizza_ai_image_region_change_grid_core::compare(&bytes[0], &bytes[1], &opts)
        .map_err(SkillError::InvalidArgs)?;
    respond_ok(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "images": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Exactly two image sources (PNG/JPEG/WebP/GIF/BMP), the before image first and the after image second. Each item has exactly one of `url` or `ref`. The first image defines the canvas the grid is measured on.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "columns": {
                        "type": "integer",
                        "default": 4,
                        "minimum": 1,
                        "maximum": 32,
                        "description": "Number of grid columns, 1-32 (default 4). Columns are labelled A, B, C ... left to right, so a cell reads like `C2`. Cannot exceed the first image's width in pixels."
                    },
                    "rows": {
                        "type": "integer",
                        "default": 4,
                        "minimum": 1,
                        "maximum": 32,
                        "description": "Number of grid rows, 1-32 (default 4). Rows are numbered 1 upward from the top. Cannot exceed the first image's height in pixels."
                    },
                    "threshold": {
                        "type": "number",
                        "default": 2.0,
                        "minimum": 0,
                        "maximum": 100,
                        "description": "Per-pixel sensitivity, 0-100 percent of the maximum possible colour difference. A pixel counts as changed when its difference is strictly greater than this. The default 2 ignores JPEG re-compression noise; use 0 to count every non-identical pixel, or raise it to 10+ to catch only obvious changes."
                    },
                    "min_change": {
                        "type": "number",
                        "default": 1.0,
                        "minimum": 0,
                        "maximum": 100,
                        "description": "Noise filter, 0-100 percent. A cell is flagged as changed only when at least this share of its pixels changed (default 1). Every cell's numbers are still reported; this only decides the `changed` flag, the changed-cell count and the ranked shortlist."
                    },
                    "metric": {
                        "type": "string",
                        "enum": ["rgb", "luma", "max-channel"],
                        "default": "rgb",
                        "description": "How each pixel's difference is measured. rgb (default) is the root-mean-square difference across red, green, blue and alpha. luma compares perceived brightness only (Rec. 601), so a recolour at the same brightness reads as unchanged. max-channel takes the single largest channel difference and is the strictest."
                    },
                    "size_mismatch": {
                        "type": "string",
                        "enum": ["resize", "error"],
                        "default": "resize",
                        "description": "What to do when the two images are different sizes. resize (default) scales the second image onto the first image's canvas before comparing; error refuses the comparison instead."
                    },
                    "map": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include an ASCII density map of the grid (one line per row, one glyph per cell, plus a legend) alongside the numbers. On by default; set false for numbers only."
                    }
                },
                "required": ["images"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
