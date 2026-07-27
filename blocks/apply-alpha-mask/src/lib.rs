//! gizza-ai/apply-alpha-mask — use a second (grayscale) image as the alpha channel
//! of the first, cutting out a transparent PNG. Chat + CLI only (two source-list
//! image inputs + image output; no standalone page, matching image-composite /
//! image-split-overlay / image-collage — the page file-input takes a single upload).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_apply_alpha_mask_core::{
    apply_mask_from_bytes, parse_channel, parse_existing_alpha, parse_fit,
};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 40 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_channel")]
    channel: String,
    #[serde(default)]
    invert: bool,
    #[serde(default = "default_fit")]
    fit: String,
    #[serde(default)]
    threshold: i64,
    #[serde(default = "default_existing_alpha")]
    existing_alpha: String,
}

fn default_channel() -> String { "luminance".to_string() }
fn default_fit() -> String { "stretch".to_string() }
fn default_existing_alpha() -> String { "replace".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 2)
                .required()
                .describe("Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): the PICTURE whose colors are kept first, the MASK image second. The mask's brightness becomes the picture's transparency — white = opaque, black = transparent. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::enumv("channel", ["luminance", "average", "red", "green", "blue", "alpha"])
                .default("luminance")
                .describe("Which value of each mask pixel becomes the alpha level: luminance (default, perceptual gray), average (mean of R/G/B), red/green/blue (a single channel), or alpha (the mask's own transparency)."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("Flip the mask so dark areas become opaque and light areas transparent. Default false (white = opaque)."),
        )
        .param(
            Param::enumv("fit", ["stretch", "cover", "contain"])
                .default("stretch")
                .describe("How the mask is resized to the picture when their sizes differ: stretch (default, ignore aspect), cover (fill, center-crop overflow), or contain (fit inside, the uncovered border stays fully transparent)."),
        )
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(255.0)
                .default(0)
                .describe("Hard cutoff, 0-255. 0 (default) keeps the mask's smooth partial transparency; any value above 0 binarizes the alpha — mask values at or above it become fully opaque, below it fully transparent."),
        )
        .param(
            Param::enumv("existing_alpha", ["replace", "multiply"])
                .default("replace")
                .describe("How the mask combines with the picture's own transparency: replace (default, the mask value becomes the alpha outright) or multiply (keep pixels transparent in EITHER the picture or the mask)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct ApplyAlphaMask;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/apply-alpha-mask",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Use a second grayscale image as the alpha channel of the first to cut out a transparent PNG.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Apply one image as the alpha (transparency) mask of another, producing a transparent PNG cutout. Provide exactly two image sources as an ordered list: the PICTURE (its RGB is kept) first, the MASK second. A value read from the mask per pixel — `channel` selects luminance (default), a single R/G/B channel, the average, or the mask's own alpha — becomes each output pixel's opacity (white = opaque, black = transparent; set `invert` to swap). `fit` (stretch/cover/contain) resizes the mask to the picture; `threshold` (0 keeps smooth alpha, >0 binarizes) hard-cuts the edge; `existing_alpha` replaces the picture's alpha or multiplies into it. Output is always PNG. Each image is a url or a `ref` (PNG/JPEG/WebP/GIF/BMP).",
        parameters = schema_json()
    ),
)]
impl ApplyAlphaMask {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("apply-alpha-mask")?;
    if args.images.len() != 2 {
        return Err(SkillError::InvalidArgs(
            "apply-alpha-mask needs exactly 2 image sources (picture first, mask second)".into(),
        ));
    }
    let channel = parse_channel(&args.channel).map_err(SkillError::InvalidArgs)?;
    let fit = parse_fit(&args.fit).map_err(SkillError::InvalidArgs)?;
    let existing = parse_existing_alpha(&args.existing_alpha).map_err(SkillError::InvalidArgs)?;
    let threshold = args.threshold.clamp(0, 255) as u8;

    let mut resolved = Vec::with_capacity(2);
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        resolved.push(bytes);
    }

    let out = apply_mask_from_bytes(
        &resolved[0], &resolved[1], channel, args.invert, fit, threshold, existing,
    )
    .map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &out,
        "image/png",
        "masked.png".to_string(),
        format!(
            "applied {} mask (fit {}{}{}) -> transparent PNG, {} bytes",
            args.channel,
            args.fit,
            if args.invert { ", inverted" } else { "" },
            if threshold > 0 { format!(", threshold {threshold}") } else { String::new() },
            out.len()
        ),
        MAX_OUTPUT_BYTES,
    )
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
                        "description": "Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): the PICTURE whose colors are kept first, the MASK image second. The mask's brightness becomes the picture's transparency — white = opaque, black = transparent. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "channel": { "type": "string", "enum": ["luminance", "average", "red", "green", "blue", "alpha"], "default": "luminance", "description": "Which value of each mask pixel becomes the alpha level: luminance (default, perceptual gray), average (mean of R/G/B), red/green/blue (a single channel), or alpha (the mask's own transparency)." },
                    "invert": { "type": "boolean", "default": false, "description": "Flip the mask so dark areas become opaque and light areas transparent. Default false (white = opaque)." },
                    "fit": { "type": "string", "enum": ["stretch", "cover", "contain"], "default": "stretch", "description": "How the mask is resized to the picture when their sizes differ: stretch (default, ignore aspect), cover (fill, center-crop overflow), or contain (fit inside, the uncovered border stays fully transparent)." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 255, "default": 0, "description": "Hard cutoff, 0-255. 0 (default) keeps the mask's smooth partial transparency; any value above 0 binarizes the alpha — mask values at or above it become fully opaque, below it fully transparent." },
                    "existing_alpha": { "type": "string", "enum": ["replace", "multiply"], "default": "replace", "description": "How the mask combines with the picture's own transparency: replace (default, the mask value becomes the alpha outright) or multiply (keep pixels transparent in EITHER the picture or the mask)." }
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
