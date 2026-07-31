//! gizza-ai/alpha-defringe — remove the dark/light/color halo (fringe) from the
//! anti-aliased edge of a cutout against transparency, producing a clean
//! transparent PNG. Pure Rust (image crate) — runs on all backends incl. the
//! chat SW. Surfaces: chat + CLI (image input + image bytes output → no page,
//! like image-opacity / normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_alpha_defringe_core::{defringe, parse_color, Mode};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_radius")]
    radius: u32,
    #[serde(default = "default_threshold")]
    threshold: u32,
    #[serde(default = "default_matte")]
    matte_color: String,
}
fn default_mode() -> String {
    "bleed".to_string()
}
fn default_radius() -> u32 {
    2
}
fn default_threshold() -> u32 {
    250
}
fn default_matte() -> String {
    "#000000".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", ["bleed", "unmatte"])
                .default("bleed")
                .describe("'bleed' (default) repaints translucent edge pixels (alpha below threshold) with the color of nearby clean pixels within radius, removing a dark/light/color halo without naming it; 'unmatte' algebraically removes a flat matte_color from translucent pixels to recover the true foreground color."),
        )
        .param(
            Param::integer("radius")
                .min(1.0)
                .max(16.0)
                .default(2)
                .describe("Bleed search radius in pixels (mode=bleed): how far to look for clean source pixels to repaint the fringe from. 1–16, default 2."),
        )
        .param(
            Param::integer("threshold")
                .min(1.0)
                .max(255.0)
                .default(250)
                .describe("Alpha (1–255) at or above which a pixel counts as a clean color source; pixels below it are the translucent edge pixels to repair. Default 250."),
        )
        .param(
            Param::string("matte_color")
                .default("#000000")
                .describe("Flat background color to remove in mode=unmatte. Accepts #rgb, #rrggbb, or a name (black/white/gray/grey/red/green/blue). Default #000000 (black)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AlphaDefringe;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/alpha-defringe",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove the halo/fringe from a cutout's edge",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Remove the dark, light, or colored halo (fringe) left on the anti-aliased edge of a cutout against transparency, producing a clean transparent PNG. mode='bleed' (default) repaints each translucent edge pixel (alpha below threshold) with the color of nearby clean pixels within radius, erasing a halo of any color without naming the old background. mode='unmatte' algebraically removes a known flat matte_color (F = (C - (1-alpha)*M)/alpha) to recover the true foreground when the cutout was anti-aliased over a solid background. The alpha channel is always preserved and the result is a PNG. radius is the bleed search width in pixels (1-16, default 2). threshold is the alpha (1-255, default 250) at/above which a pixel is a clean source. matte_color accepts #rgb, #rrggbb, or black/white/gray/red/green/blue. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl AlphaDefringe {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("alpha-defringe")?;
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let matte = parse_color(&args.matte_color).map_err(SkillError::InvalidArgs)?;
    let radius = args.radius.clamp(1, 16);
    let threshold = args.threshold.clamp(1, 255) as u8;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = defringe(&bytes, mode, radius, threshold, matte).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "defringed.png".to_string(),
        format!(
            "Defringed ({} mode) — {} bytes PNG",
            args.mode,
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
                    "url":         { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":        { "type": "string", "enum": ["bleed", "unmatte"], "default": "bleed", "description": "'bleed' (default) repaints translucent edge pixels (alpha below threshold) with the color of nearby clean pixels within radius, removing a dark/light/color halo without naming it; 'unmatte' algebraically removes a flat matte_color from translucent pixels to recover the true foreground color." },
                    "radius":      { "type": "integer", "minimum": 1, "maximum": 16, "default": 2, "description": "Bleed search radius in pixels (mode=bleed): how far to look for clean source pixels to repaint the fringe from. 1–16, default 2." },
                    "threshold":   { "type": "integer", "minimum": 1, "maximum": 255, "default": 250, "description": "Alpha (1–255) at or above which a pixel counts as a clean color source; pixels below it are the translucent edge pixels to repair. Default 250." },
                    "matte_color": { "type": "string", "default": "#000000", "description": "Flat background color to remove in mode=unmatte. Accepts #rgb, #rrggbb, or a name (black/white/gray/grey/red/green/blue). Default #000000 (black)." }
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
