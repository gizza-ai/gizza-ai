//! gizza-ai/histogram-equalizer — equalize an image's histogram to boost
//! contrast (global HE or adaptive CLAHE). Returns a PNG. Pure-Rust (image
//! crate). Surfaces: chat + CLI (image input + image bytes output → no page,
//! like colorblind-simulator / normalize-image).
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI); the drift-guard test below proves it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, filename_with_suffix, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_histogram_equalizer_core::{
    equalize, ChannelMode, Method, CHANNEL_MODES, CLIP_LIMIT_MAX, CLIP_LIMIT_MIN,
    DEFAULT_CHANNEL_MODE, DEFAULT_CLIP_LIMIT, DEFAULT_METHOD, DEFAULT_TILE_GRID, METHODS,
    TILE_GRID_MAX, TILE_GRID_MIN,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_channel_mode")]
    channel_mode: String,
    #[serde(default)]
    clip_limit: Option<f64>,
    #[serde(default)]
    tile_grid: Option<f64>,
}
fn default_method() -> String {
    DEFAULT_METHOD.to_string()
}
fn default_channel_mode() -> String {
    DEFAULT_CHANNEL_MODE.to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("method", METHODS)
                .default(DEFAULT_METHOD)
                .describe(
                    "Equalization method: adaptive (default, CLAHE — tiled local equalization with \
                     contrast limiting, best for photos and uneven lighting) or global (one \
                     histogram CDF mapping applied to the whole image).",
                ),
        )
        .param(
            Param::enumv("channel_mode", CHANNEL_MODES)
                .default(DEFAULT_CHANNEL_MODE)
                .describe(
                    "Which channels to equalize: luminance (default — equalize a luma channel and \
                     rescale RGB, preserving colour), per_channel (equalize R, G and B \
                     independently, can shift colour), or grayscale (output a grayscale image of \
                     the equalized luma). Alpha is always preserved.",
                ),
        )
        .param(
            Param::number("clip_limit")
                .min(CLIP_LIMIT_MIN)
                .max(CLIP_LIMIT_MAX)
                .default(DEFAULT_CLIP_LIMIT)
                .describe(
                    "Contrast limit for adaptive/CLAHE, 1-40 (default 2). Caps each tile's \
                     histogram slope to curb noise amplification; higher allows more local \
                     contrast. Ignored by the global method.",
                ),
        )
        .param(
            Param::integer("tile_grid")
                .min(TILE_GRID_MIN as f64)
                .max(TILE_GRID_MAX as f64)
                .default(DEFAULT_TILE_GRID)
                .describe(
                    "Tiles per axis for adaptive/CLAHE, 1-32 (default 8). The image is split into \
                     tile_grid x tile_grid regions, each equalized then bilinearly blended; 1 is \
                     effectively whole-image adaptive. Ignored by the global method.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HistogramEqualizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/histogram-equalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Equalize an image's histogram to boost contrast, with global or adaptive CLAHE.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Equalize an image's histogram to improve contrast. method = adaptive (CLAHE, default) or global; channel_mode = luminance (default), per_channel or grayscale; clip_limit 1-40 (default 2) limits adaptive contrast; tile_grid 1-32 (default 8) sets the adaptive tile count. Alpha is preserved. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl HistogramEqualizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("histogram-equalizer")?;
    let method = Method::parse(Some(&args.method)).map_err(SkillError::InvalidArgs)?;
    let channel_mode = ChannelMode::parse(Some(&args.channel_mode)).map_err(SkillError::InvalidArgs)?;
    let clip_limit = args.clip_limit.unwrap_or(DEFAULT_CLIP_LIMIT);
    let tile_grid = args.tile_grid.unwrap_or(DEFAULT_TILE_GRID as f64);

    let (bytes, _mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = equalize(&bytes, method, channel_mode, clip_limit, tile_grid)
        .map_err(SkillError::InvalidArgs)?;

    let filename = filename_with_suffix(&in_name, "-equalized", "png");
    let for_llm = format!(
        "equalized {in_name} ({} {} equalization, {} bytes PNG)",
        args.method,
        args.channel_mode,
        png.len()
    );
    build_media_envelope(&png, "image/png", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema below, so the LLM-facing tool definition never silently changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "method": {
                        "type": "string",
                        "enum": ["adaptive", "global"],
                        "default": "adaptive",
                        "description": "Equalization method: adaptive (default, CLAHE — tiled local equalization with contrast limiting, best for photos and uneven lighting) or global (one histogram CDF mapping applied to the whole image)."
                    },
                    "channel_mode": {
                        "type": "string",
                        "enum": ["luminance", "per_channel", "grayscale"],
                        "default": "luminance",
                        "description": "Which channels to equalize: luminance (default — equalize a luma channel and rescale RGB, preserving colour), per_channel (equalize R, G and B independently, can shift colour), or grayscale (output a grayscale image of the equalized luma). Alpha is always preserved."
                    },
                    "clip_limit": {
                        "type": "number",
                        "minimum": 1,
                        "maximum": 40,
                        "default": 2.0,
                        "description": "Contrast limit for adaptive/CLAHE, 1-40 (default 2). Caps each tile's histogram slope to curb noise amplification; higher allows more local contrast. Ignored by the global method."
                    },
                    "tile_grid": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 32,
                        "default": 8,
                        "description": "Tiles per axis for adaptive/CLAHE, 1-32 (default 8). The image is split into tile_grid x tile_grid regions, each equalized then bilinearly blended; 1 is effectively whole-image adaptive. Ignored by the global method."
                    }
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
    fn descriptor_exposes_every_method_and_mode() {
        let json = schema_json();
        for name in METHODS {
            assert!(json.contains(name), "schema must list method {name}");
        }
        for name in CHANNEL_MODES {
            assert!(json.contains(name), "schema must list channel_mode {name}");
        }
    }
}
