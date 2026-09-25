//! gizza-ai/image-oil-painting — repaint a photo as an oil painting: a local
//! intensity-histogram brush filter lays down flat daubs of pigment, a seeded
//! bristle warp keeps the strokes from looking machine-made, and an optional
//! linen weave finishes the canvas. Returns a PNG.
//!
//! Deterministic and non-ML — no model, no network, no randomness beyond the
//! seed. Pure Rust (`image`), so it runs on all backends incl. the chat SW.
//! Surfaces: chat + CLI (image input + image bytes output → no page, like
//! image-low-poly / image-to-pixel-art / image-to-sketch).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_oil_painting_core::{oil_painting, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_radius")]
    radius: u64,
    #[serde(default = "default_intensity_levels")]
    intensity_levels: u64,
    #[serde(default = "default_brush_strength")]
    brush_strength: f64,
    #[serde(default = "default_saturation")]
    saturation: f64,
    #[serde(default)]
    canvas_texture: f64,
    #[serde(default = "default_seed")]
    seed: i64,
}
fn default_radius() -> u64 {
    4
}
fn default_intensity_levels() -> u64 {
    24
}
fn default_brush_strength() -> f64 {
    0.85
}
fn default_saturation() -> f64 {
    1.1
}
fn default_seed() -> i64 {
    1
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("radius")
                .min(1.0)
                .max(12.0)
                .default(4)
                .describe("Brush width in pixels, 1-12 (default 4). Each pixel is repainted from the neighbourhood this far around it, so higher = broader, bolder daubs and less fine detail; 1-2 keeps faces and text readable, 8-12 gives a loose palette-knife look."),
        )
        .param(
            Param::integer("intensity_levels")
                .min(8.0)
                .max(64.0)
                .default(24)
                .describe("How many brightness buckets the brush sorts each neighbourhood into, 8-64 (default 24). Fewer buckets merge more pixels into one stroke for a chunkier, more graphic painting; more buckets keep the result closer to the photo."),
        )
        .param(
            Param::number("brush_strength")
                .min(0.0)
                .max(1.0)
                .default(0.85)
                .describe("How much of the painted result replaces the photo, 0-1 (default 0.85). 1 = full impasto; 0.5-0.8 blends photographic detail back in for a softer glaze; 0 returns the image unpainted."),
        )
        .param(
            Param::number("saturation")
                .min(0.5)
                .max(2.0)
                .default(1.1)
                .describe("Colour boost applied before painting, 0.5-2.0 (default 1.1, 1.0 = unchanged). Oil pigment reads more vivid than a photograph; try 1.3-1.6 for a bold gallery look or 0.7 for a muted, aged one."),
        )
        .param(
            Param::number("canvas_texture")
                .min(0.0)
                .max(1.0)
                .default(0)
                .describe("Strength of a procedural linen-weave canvas overlay, 0-1 (default 0 = off). Try 0.3-0.6 to make the result read as paint on cloth rather than paint on screen."),
        )
        .param(
            Param::integer("seed")
                .default(1)
                .describe("Seed for the bristle-drag texture and the canvas weave (default 1). Same seed and settings always give the identical painting; change it to repaint the same photo with different brushwork."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageOilPainting;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-oil-painting",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a photo into an oil painting",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Repaint a photo as an oil painting with a classic non-ML brush-stroke filter: every pixel is replaced by the dominant colour of its local neighbourhood, which flattens smooth areas into daubs of pigment while keeping contours crisp, then a seeded bristle drag and an optional linen weave finish the canvas. radius is the brush width 1-12 px (default 4; higher = bolder, less detail); intensity_levels 8-64 (default 24) sets how many brightness buckets a stroke merges, fewer = chunkier and more graphic; brush_strength 0-1 (default 0.85) blends the painting over the photo, 1 = full impasto and 0 = untouched; saturation 0.5-2.0 (default 1.1) boosts colour before painting; canvas_texture 0-1 (default 0 = off) overlays a linen weave; seed repaints the same settings with different brushwork. Deterministic - same input and settings always give the same picture. Returns a PNG at the original dimensions with transparency preserved; images above 30 megapixels are rejected. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageOilPainting {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-oil-painting")?;
    let opts = Options {
        radius: args.radius.clamp(1, 12) as u32,
        intensity_levels: args.intensity_levels.clamp(8, 64) as u32,
        brush_strength: args.brush_strength.clamp(0.0, 1.0) as f32,
        saturation: args.saturation.clamp(0.5, 2.0) as f32,
        canvas_texture: args.canvas_texture.clamp(0.0, 1.0) as f32,
        seed: args.seed as u64,
    };
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = oil_painting(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "oil-painting.png".to_string(),
        format!(
            "oil painting (brush radius {}px, {} intensity levels, strength {}, saturation {}, {} bytes PNG)",
            opts.radius,
            opts.intensity_levels,
            opts.brush_strength,
            opts.saturation,
            png.len()
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
                    "url":              { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":              { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "radius":           { "type": "integer", "minimum": 1, "maximum": 12, "default": 4, "description": "Brush width in pixels, 1-12 (default 4). Each pixel is repainted from the neighbourhood this far around it, so higher = broader, bolder daubs and less fine detail; 1-2 keeps faces and text readable, 8-12 gives a loose palette-knife look." },
                    "intensity_levels": { "type": "integer", "minimum": 8, "maximum": 64, "default": 24, "description": "How many brightness buckets the brush sorts each neighbourhood into, 8-64 (default 24). Fewer buckets merge more pixels into one stroke for a chunkier, more graphic painting; more buckets keep the result closer to the photo." },
                    "brush_strength":   { "type": "number", "minimum": 0, "maximum": 1, "default": 0.85, "description": "How much of the painted result replaces the photo, 0-1 (default 0.85). 1 = full impasto; 0.5-0.8 blends photographic detail back in for a softer glaze; 0 returns the image unpainted." },
                    "saturation":       { "type": "number", "minimum": 0.5, "maximum": 2, "default": 1.1, "description": "Colour boost applied before painting, 0.5-2.0 (default 1.1, 1.0 = unchanged). Oil pigment reads more vivid than a photograph; try 1.3-1.6 for a bold gallery look or 0.7 for a muted, aged one." },
                    "canvas_texture":   { "type": "number", "minimum": 0, "maximum": 1, "default": 0, "description": "Strength of a procedural linen-weave canvas overlay, 0-1 (default 0 = off). Try 0.3-0.6 to make the result read as paint on cloth rather than paint on screen." },
                    "seed":             { "type": "integer", "default": 1, "description": "Seed for the bristle-drag texture and the canvas weave (default 1). Same seed and settings always give the identical painting; change it to repaint the same photo with different brushwork." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The serde defaults must match what the schema advertises — a mismatch
    /// means chat sees one default and the handler applies another.
    #[test]
    fn serde_defaults_match_the_schema_defaults() {
        let args: Args = serde_json::from_str(r#"{"url":"https://example.com/a.png"}"#).unwrap();
        assert_eq!(args.radius, 4);
        assert_eq!(args.intensity_levels, 24);
        assert_eq!(args.brush_strength, 0.85);
        assert_eq!(args.saturation, 1.1);
        assert_eq!(args.canvas_texture, 0.0);
        assert_eq!(args.seed, 1);
    }
}
