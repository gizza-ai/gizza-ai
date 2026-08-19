//! gizza-ai/color-transfer — recolor a target photo with the color mood of a
//! reference photo by matching channel statistics. Chat + CLI only (source-list
//! input + image output; no standalone page, matching image-composite/image-collage).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_color_transfer_core::{parse_format, parse_method, transfer_from_bytes, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 40 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_strength")]
    strength: f64,
    #[serde(default)]
    preserve_luminance: bool,
    #[serde(default = "default_saturation")]
    saturation: f64,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_quality")]
    quality: i64,
}

fn default_method() -> String { "lab-stats".to_string() }
fn default_strength() -> f64 { 100.0 }
fn default_saturation() -> f64 { 100.0 }
fn default_format() -> String { "png".to_string() }
fn default_quality() -> i64 { 90 }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 2)
                .required()
                .describe("Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): the target photo to recolor first, the reference photo whose colors you want second. The output keeps the target's size and transparency; the reference is only sampled for its colors. Each item has exactly one of `url` or `ref`."),
        )
        .param(
            Param::enumv("method", ["lab-stats", "rgb-stats", "histogram", "mean-only"])
                .default("lab-stats")
                .describe("How the reference's colors are matched: lab-stats (default, Reinhard-style mean+standard-deviation match in perceptual CIELAB — natural results), rgb-stats (same match per red/green/blue channel — punchier, more literal), histogram (full per-channel histogram match — strongest, best for film/LUT-style looks), mean-only (shift the color cast only and keep the target's own contrast — gentle white-balance style)."),
        )
        .param(
            Param::number("strength")
                .min(0.0)
                .max(100.0)
                .default(100.0)
                .describe("How far to push the recolor, 0-100 percent. 100 (default) is the full transfer; 50 blends it half-way with the original; 0 returns the target untouched."),
        )
        .param(
            Param::boolean("preserve_luminance")
                .default(false)
                .describe("Keep the target's own lightness and take only the reference's color (hue/chroma). Use it when the reference is much brighter or darker than the target and you only want its color mood. Default false."),
        )
        .param(
            Param::number("saturation")
                .min(0.0)
                .max(200.0)
                .default(100.0)
                .describe("Scale the color intensity of the result, 0-200 percent. 100 (default) leaves the transfer as-is, 0 makes it black and white, 150 boosts it. Applied after the transfer."),
        )
        .param(
            Param::enumv("format", ["png", "jpeg"])
                .default("png")
                .describe("Output image format: png (default, lossless and keeps transparency) or jpeg (smaller, flattens transparency to black)."),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(90)
                .describe("JPEG quality 1-100 (higher = better and larger). Default 90. Ignored when format is png."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct ColorTransfer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-transfer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Recolor a photo with the color mood of a reference image by matching channel statistics.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Transfer the color mood of a reference photo onto a target photo by matching their channel statistics. The first image source is the target that gets recolored (it defines the output size and keeps its transparency); the second is the reference whose colors are copied. `method` picks the match: lab-stats (default, Reinhard mean+standard-deviation in CIELAB), rgb-stats, histogram (strongest), or mean-only (color cast only). `strength` (0-100) blends the result with the original, `preserve_luminance` keeps the target's lightness, `saturation` (0-200) scales the result's color intensity, and `format` outputs png or jpeg at `quality`. Provide images as an ordered list of two, each a url or a `ref` (PNG/JPEG/WebP/GIF/BMP).",
        parameters = schema_json()
    ),
)]
impl ColorTransfer {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("color-transfer")?;
    if args.images.len() != 2 {
        return Err(SkillError::InvalidArgs(
            "color-transfer needs exactly 2 image sources (target photo first, reference photo second)".into(),
        ));
    }
    let method = parse_method(&args.method).map_err(SkillError::InvalidArgs)?;
    let format = parse_format(&args.format).map_err(SkillError::InvalidArgs)?;
    let opts = Options {
        method,
        strength: args.strength.clamp(0.0, 100.0),
        preserve_luminance: args.preserve_luminance,
        saturation: args.saturation.clamp(0.0, 200.0),
        format,
        quality: args.quality.clamp(1, 100) as u8,
    };

    let mut resolved = Vec::with_capacity(2);
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        resolved.push(bytes);
    }

    let out = transfer_from_bytes(&resolved[0], &resolved[1], opts)
        .map_err(SkillError::InvalidArgs)?;

    build_media_envelope(
        &out,
        format.mime(),
        format!("color-transfer.{}", format.ext()),
        format!(
            "recolored the target with the reference's colors ({} method, strength {:.0}%, saturation {:.0}%{}) — {} bytes {}",
            args.method,
            opts.strength,
            opts.saturation,
            if opts.preserve_luminance { ", original lightness kept" } else { "" },
            out.len(),
            format.ext()
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
                        "description": "Exactly two image sources (PNG/JPEG/WebP/GIF/BMP): the target photo to recolor first, the reference photo whose colors you want second. The output keeps the target's size and transparency; the reference is only sampled for its colors. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "method": { "type": "string", "enum": ["lab-stats", "rgb-stats", "histogram", "mean-only"], "default": "lab-stats", "description": "How the reference's colors are matched: lab-stats (default, Reinhard-style mean+standard-deviation match in perceptual CIELAB — natural results), rgb-stats (same match per red/green/blue channel — punchier, more literal), histogram (full per-channel histogram match — strongest, best for film/LUT-style looks), mean-only (shift the color cast only and keep the target's own contrast — gentle white-balance style)." },
                    "strength": { "type": "number", "minimum": 0, "maximum": 100, "default": 100.0, "description": "How far to push the recolor, 0-100 percent. 100 (default) is the full transfer; 50 blends it half-way with the original; 0 returns the target untouched." },
                    "preserve_luminance": { "type": "boolean", "default": false, "description": "Keep the target's own lightness and take only the reference's color (hue/chroma). Use it when the reference is much brighter or darker than the target and you only want its color mood. Default false." },
                    "saturation": { "type": "number", "minimum": 0, "maximum": 200, "default": 100.0, "description": "Scale the color intensity of the result, 0-200 percent. 100 (default) leaves the transfer as-is, 0 makes it black and white, 150 boosts it. Applied after the transfer." },
                    "format": { "type": "string", "enum": ["png", "jpeg"], "default": "png", "description": "Output image format: png (default, lossless and keeps transparency) or jpeg (smaller, flattens transparency to black)." },
                    "quality": { "type": "integer", "minimum": 1, "maximum": 100, "default": 90, "description": "JPEG quality 1-100 (higher = better and larger). Default 90. Ignored when format is png." }
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
