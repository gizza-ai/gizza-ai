//! gizza-ai/error-level-analysis — Error Level Analysis (ELA) image forensics.
//!
//! Recompresses an image as JPEG at a known quality, subtracts the result from the
//! original, and amplifies the difference. Regions with a different compression
//! history (splices, paste-ins, painted-over areas, added text) stand out brighter
//! than the uniformly-compressed background — a classic way to spot photo edits.
//!
//! Pure-Rust (the `image` crate in core), so unlike the ffmpeg tools it runs on ALL
//! backends including the chat Service Worker. Pipeline: resolve the input image
//! (URL fetch or attachment ref) → `core::analyze` produces the amplified difference
//! map → base64 PNG envelope (lossless so the map is exact).
//!
//! Surfaces: chat + CLI. No standalone page — a pure-Rust image-bytes output has no
//! page render mode (same as lsb-embed / add-text-to-image / code-screenshot).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{build_media_envelope, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default = "default_quality")]
    quality: i64,
    #[serde(default = "default_scale")]
    scale: i64,
    #[serde(default)]
    normalize: bool,
    #[serde(default)]
    grayscale: bool,
}
fn default_quality() -> i64 {
    90
}
fn default_scale() -> i64 {
    15
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("quality")
                .default(90)
                .min(1.0)
                .max(100.0)
                .describe("JPEG recompression quality, 1-100 (default 90). The standard ELA range is 90-95: high enough that genuinely-unedited regions barely change."),
        )
        .param(
            Param::integer("scale")
                .default(15)
                .min(1.0)
                .max(100.0)
                .describe("Brightness multiplier applied to the raw difference, 1-100 (default 15). Raw ELA differences are tiny, so scaling makes them visible. Ignored when normalize is true."),
        )
        .param(
            Param::boolean("normalize")
                .default(false)
                .describe("Auto-contrast: stretch the brightest difference pixel to 255 instead of using a fixed scale. Useful when the right scale is unknown."),
        )
        .param(
            Param::boolean("grayscale")
                .default(false)
                .describe("Collapse the difference map to luminance (often easier to read than the per-channel colour map)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Ela;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/error-level-analysis",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reveal edited regions of an image with Error Level Analysis (ELA)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Run Error Level Analysis (ELA) on an image to reveal edited, spliced, or pasted-in regions. The image is recompressed as JPEG at quality (1-100, default 90) and the difference from the original is amplified — areas with a different compression history (edits, splices, added text) appear brighter than the uniformly-compressed background. Use scale (1-100, default 15) to set the brightness multiplier, normalize=true for auto-contrast, and grayscale=true for a luminance-only map. Returns a lossless PNG visualisation. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = schema_json()
    ),
)]
impl Ela {
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

    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("error-level-analysis: {e}")))?;
    if !(1..=100).contains(&args.quality) {
        return Err(SkillError::InvalidArgs(
            "quality must be between 1 and 100".into(),
        ));
    }
    if !(1..=100).contains(&args.scale) {
        return Err(SkillError::InvalidArgs(
            "scale must be between 1 and 100".into(),
        ));
    }

    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let png = gizza_ai_error_level_analysis_core::analyze(
        &bytes,
        args.quality as u8,
        args.scale as u8,
        args.normalize,
        args.grayscale,
    )
    .map_err(SkillError::InvalidArgs)?;

    let stem = in_filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(&in_filename);
    let filename = format!("{stem}-ela.png");
    let for_llm = format!(
        "ran Error Level Analysis on {in_filename} (quality {}, scale {}{}{}) → {}-byte PNG map ({filename}); brighter regions indicate a different compression history (possible edits)",
        args.quality,
        args.scale,
        if args.normalize { ", normalized" } else { "" },
        if args.grayscale { ", grayscale" } else { "" },
        png.len()
    );
    build_media_envelope(&png, "image/png", filename, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "quality":   { "type": "integer", "minimum": 1, "maximum": 100, "default": 90, "description": "JPEG recompression quality, 1-100 (default 90). The standard ELA range is 90-95: high enough that genuinely-unedited regions barely change." },
                    "scale":     { "type": "integer", "minimum": 1, "maximum": 100, "default": 15, "description": "Brightness multiplier applied to the raw difference, 1-100 (default 15). Raw ELA differences are tiny, so scaling makes them visible. Ignored when normalize is true." },
                    "normalize": { "type": "boolean", "default": false, "description": "Auto-contrast: stretch the brightest difference pixel to 255 instead of using a fixed scale. Useful when the right scale is unknown." },
                    "grayscale": { "type": "boolean", "default": false, "description": "Collapse the difference map to luminance (often easier to read than the per-channel colour map)." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
