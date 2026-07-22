//! gizza-ai/multi-photo-scan-splitter — fetch one flatbed scan holding several
//! photos (URL or attachment ref), detect + auto-straighten + crop each photo,
//! and return them bundled in a ZIP.
//!
//! Pipeline: resolve the source image → `core::split_photos` (pure-Rust `image`
//! + `zip`: threshold vs the scanner background → connected components →
//! min-area rectangle deskew → crop → encode) → base64 envelope (the ZIP as a
//! downloadable file).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the ffmpeg media page shape — the same "no-page file-input"
//! pattern as spritesheet-slice / extract-pdf-images / encrypt-file).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_multi_photo_scan_splitter_core::Summary;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 24 * 1024 * 1024; // 24 MiB — a 600-dpi flatbed scan.

fn default_true() -> bool {
    true
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    background: Option<String>,
    #[serde(default = "default_true")]
    straighten: bool,
    #[serde(default)]
    min_size: Option<u32>,
    #[serde(default)]
    edge_trim: u32,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    max_photos: Option<usize>,
}

/// `Input::Image` emits the `url`⊕`ref` `oneOf`; every knob maps 1:1 to
/// `core::SplitParams`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("background", ["auto", "white", "black"])
                .default("auto")
                .describe(
                    "Scanner bed colour. auto samples the scan border; set white or black to force it.",
                ),
        )
        .param(
            Param::boolean("straighten")
                .default(true)
                .describe("Auto-rotate each detected photo upright to remove scan skew (deskew)."),
        )
        .param(
            Param::integer("min_size")
                .min(1.0)
                .default(48)
                .describe(
                    "Ignore detected regions whose width OR height is under this many pixels (dust/speckle).",
                ),
        )
        .param(
            Param::integer("edge_trim")
                .min(0.0)
                .default(0)
                .describe("Trim this many pixels inward on every side of each crop to shave bed bleed."),
        )
        .param(
            Param::enumv("format", ["png", "jpeg", "webp", "bmp"])
                .default("png")
                .describe("Per-photo image format. png/webp/bmp are lossless; jpeg is smaller/opaque."),
        )
        .param(
            Param::string("prefix")
                .default("photo")
                .describe("Filename base for each photo, e.g. photo -> photo_1.png, photo_2.png."),
        )
        .param(
            Param::integer("max_photos")
                .min(1.0)
                .describe("Keep only the this-many largest photos (default: every photo detected)."),
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
    let straighten_note = if s.straightened {
        "straightened"
    } else {
        "not straightened"
    };
    format!(
        "Split {in_filename} into {} photo(s) on a {} background ({straighten_note}): {dims} → {out_filename} ({zip_len}-byte ZIP).",
        s.photos, s.background
    )
}

#[cfg(target_arch = "wasm32")]
struct MultiPhotoScanSplitter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/multi-photo-scan-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a multi-photo flatbed scan into separate straightened images (ZIP)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Detect the separate photos on one flatbed scan, auto-straighten (deskew) and crop each, and return them bundled in a ZIP. background is the scanner bed colour (auto samples the scan border, or force white/black). straighten (default true) rotates each photo upright. min_size ignores regions smaller than this many pixels in either dimension (dust/speckle). edge_trim shaves that many pixels inward on every side of each crop. format sets the per-photo encoding (png/jpeg/webp/bmp), prefix names the files (photo -> photo_1.png, photo_2.png, …), and max_photos keeps only the largest N. Works when the photos have a clear gap between them and the bed contrasts with them; touching photos merge into one region. Provide the scan as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl MultiPhotoScanSplitter {
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
    use gizza_ai_multi_photo_scan_splitter_core::{Background, OutFormat, SplitParams};

    let args: Args = serde_json::from_slice(&body).invalid_args("multi-photo-scan-splitter")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let background = Background::parse(args.background.as_deref().unwrap_or("auto"))
        .map_err(SkillError::InvalidArgs)?;
    let format =
        OutFormat::parse(args.format.as_deref().unwrap_or("png")).map_err(SkillError::InvalidArgs)?;
    let params = SplitParams {
        background,
        straighten: args.straighten,
        min_size: args.min_size.unwrap_or(48),
        edge_trim: args.edge_trim,
        format,
        prefix: args.prefix.unwrap_or_else(|| "photo".to_string()),
        max_photos: args.max_photos,
    };

    let (zip, summary) = gizza_ai_multi_photo_scan_splitter_core::split_photos(&bytes, &params)
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
                    "background": { "type": "string", "enum": ["auto", "white", "black"], "default": "auto", "description": "Scanner bed colour. auto samples the scan border; set white or black to force it." },
                    "straighten": { "type": "boolean", "default": true, "description": "Auto-rotate each detected photo upright to remove scan skew (deskew)." },
                    "min_size": { "type": "integer", "minimum": 1, "default": 48, "description": "Ignore detected regions whose width OR height is under this many pixels (dust/speckle)." },
                    "edge_trim": { "type": "integer", "minimum": 0, "default": 0, "description": "Trim this many pixels inward on every side of each crop to shave bed bleed." },
                    "format": { "type": "string", "enum": ["png", "jpeg", "webp", "bmp"], "default": "png", "description": "Per-photo image format. png/webp/bmp are lossless; jpeg is smaller/opaque." },
                    "prefix": { "type": "string", "default": "photo", "description": "Filename base for each photo, e.g. photo -> photo_1.png, photo_2.png." },
                    "max_photos": { "type": "integer", "minimum": 1, "description": "Keep only the this-many largest photos (default: every photo detected)." }
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
        assert_eq!(replace_extension("family-scan.png", "zip"), "family-scan.zip");
    }

    #[test]
    fn summary_line_is_exact_and_useful() {
        let s = Summary {
            photos: 2,
            background: "white",
            straightened: true,
            sizes: vec![(120, 80), (100, 70)],
        };
        assert_eq!(
            summary_line("scan.png", "scan.zip", 4096, &s),
            "Split scan.png into 2 photo(s) on a white background (straightened): 120x80, 100x70 → scan.zip (4096-byte ZIP)."
        );
    }
}
