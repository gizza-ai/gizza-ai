//! gizza-ai/collage-splitter — fetch one photo-grid collage (URL or attachment
//! ref), detect the grid cells (or split by an explicit rows × columns), crop
//! each photo back out, and return them bundled in a ZIP.
//!
//! Pipeline: resolve the source image → `core::split_collage` (pure-Rust
//! `image` + `zip`: gutter-line detection or equal-split → crop each cell →
//! encode) → base64 envelope (the ZIP as a downloadable file).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the ffmpeg media page shape — the same "no-page file-input"
//! pattern as multi-photo-scan-splitter / extract-pdf-images / encrypt-file).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_collage_splitter_core::Summary;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 24 * 1024 * 1024; // 24 MiB — a high-res collage export.

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    rows: u32,
    #[serde(default)]
    columns: u32,
    #[serde(default)]
    gutter: Option<String>,
    #[serde(default)]
    trim: u32,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf`; every knob maps 1:1 to
/// `core::SplitParams`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("rows")
                .min(0.0)
                .max(20.0)
                .default(0)
                .describe(
                    "Number of grid rows. 0 (default) auto-detects rows from the gutters; set 1-20 to force an even split.",
                ),
        )
        .param(
            Param::integer("columns")
                .min(0.0)
                .max(20.0)
                .default(0)
                .describe(
                    "Number of grid columns. 0 (default) auto-detects columns from the gutters; set 1-20 to force an even split.",
                ),
        )
        .param(
            Param::enumv("gutter", ["auto", "white", "black"])
                .default("auto")
                .describe(
                    "Gutter/border colour used to auto-detect the grid. auto samples the outer border; set white or black to force it.",
                ),
        )
        .param(
            Param::integer("trim")
                .min(0.0)
                .default(0)
                .describe("Trim this many pixels inward on every side of each cell to shave leftover border bleed."),
        )
        .param(
            Param::enumv("format", ["png", "jpeg", "webp", "bmp"])
                .default("png")
                .describe("Per-cell image format. png/webp/bmp are lossless; jpeg is smaller/opaque."),
        )
        .param(
            Param::string("prefix")
                .default("cell")
                .describe("Filename base for each cell, e.g. cell -> cell_1.png, cell_2.png."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// One exact, useful LLM/CLI line describing what was produced.
fn summary_line(in_filename: &str, out_filename: &str, zip_len: usize, s: &Summary) -> String {
    let dims = s
        .sizes
        .iter()
        .map(|(w, h)| format!("{w}x{h}"))
        .collect::<Vec<_>>()
        .join(", ");
    let how = if s.auto_detected {
        "auto-detected"
    } else {
        "even split"
    };
    format!(
        "Split {in_filename} into a {}x{} grid ({} cell(s), {how}, {} gutter): {dims} → {out_filename} ({zip_len}-byte ZIP).",
        s.rows, s.columns, s.cells, s.gutter
    )
}

#[cfg(target_arch = "wasm32")]
struct CollageSplitter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/collage-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect the cells of a photo-grid collage and export each photo as its own image (ZIP)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Detect the cells of a photo-grid collage (an Instagram/grid-maker/MidJourney layout) and export each photo back out individually, bundled in a ZIP. rows and columns set the grid size: 0 (default) auto-detects that axis from the gutters between photos; set 1-20 to force an even split (e.g. rows=2 columns=2 for a MidJourney 2x2 grid, or rows=3 columns=3 for a 3x3). gutter is the border colour used for auto-detection (auto samples the outer border, or force white/black). trim shaves that many pixels inward on every side of each cell to remove leftover border bleed. format sets the per-cell encoding (png/jpeg/webp/bmp) and prefix names the files (cell -> cell_1.png, cell_2.png, … in row-major reading order). Assumes a grid layout separated by roughly-uniform gutters; photos with large flat gutter-coloured areas can confuse auto-detection — pass rows/columns explicitly then. For arbitrarily-placed, possibly-rotated photos on a flatbed scan, use multi-photo-scan-splitter instead. Provide the collage as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl CollageSplitter {
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
    use gizza_ai_collage_splitter_core::{Gutter, OutFormat, SplitParams};

    let args: Args = serde_json::from_slice(&body).invalid_args("collage-splitter")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let gutter = Gutter::parse(args.gutter.as_deref().unwrap_or("auto"))
        .map_err(SkillError::InvalidArgs)?;
    let format =
        OutFormat::parse(args.format.as_deref().unwrap_or("png")).map_err(SkillError::InvalidArgs)?;
    let params = SplitParams {
        rows: args.rows,
        columns: args.columns,
        gutter,
        trim: args.trim,
        format,
        prefix: args.prefix.unwrap_or_else(|| "cell".to_string()),
    };

    let (zip, summary) = gizza_ai_collage_splitter_core::split_collage(&bytes, &params)
        .map_err(SkillError::InvalidArgs)?;

    let filename = replace_extension(&in_filename, "zip");
    let zip_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");
    let for_llm = summary_line(&in_filename, &filename, zip_len, &summary);

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/zip".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + the splitter knobs).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "rows": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0, "description": "Number of grid rows. 0 (default) auto-detects rows from the gutters; set 1-20 to force an even split." },
                    "columns": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0, "description": "Number of grid columns. 0 (default) auto-detects columns from the gutters; set 1-20 to force an even split." },
                    "gutter": { "type": "string", "enum": ["auto", "white", "black"], "default": "auto", "description": "Gutter/border colour used to auto-detect the grid. auto samples the outer border; set white or black to force it." },
                    "trim": { "type": "integer", "minimum": 0, "default": 0, "description": "Trim this many pixels inward on every side of each cell to shave leftover border bleed." },
                    "format": { "type": "string", "enum": ["png", "jpeg", "webp", "bmp"], "default": "png", "description": "Per-cell image format. png/webp/bmp are lossless; jpeg is smaller/opaque." },
                    "prefix": { "type": "string", "default": "cell", "description": "Filename base for each cell, e.g. cell -> cell_1.png, cell_2.png." }
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
        assert_eq!(replace_extension("grid.png", "zip"), "grid.zip");
    }

    #[test]
    fn summary_line_is_exact_and_useful() {
        let s = Summary {
            rows: 2,
            columns: 3,
            cells: 6,
            gutter: "white",
            auto_detected: true,
            sizes: vec![(100, 100), (100, 100), (100, 100), (100, 100), (100, 100), (100, 100)],
        };
        assert_eq!(
            summary_line("grid.png", "grid.zip", 4096, &s),
            "Split grid.png into a 2x3 grid (6 cell(s), auto-detected, white gutter): 100x100, 100x100, 100x100, 100x100, 100x100, 100x100 → grid.zip (4096-byte ZIP)."
        );
    }
}
