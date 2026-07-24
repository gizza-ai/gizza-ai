//! gizza-ai/image-average-color — compute the single mean color of an image.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::average` (decode + mean
//! in both sRGB and linear light) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report — the F3 no-page
//! file-input pattern, like image-info / image-color-picker / color-palette-extract
//! report tools; the generator's pure-tool page can't hand uploaded bytes to a
//! wasm decoder).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Exclude (near-)transparent pixels from the mean. Defaults to true.
    #[serde(default)]
    ignore_transparency: Option<bool>,
}

/// One mean color in several notations (mirrors core::MeanColor).
#[derive(Serialize)]
struct Mean {
    hex: String,
    hex_rgba: String,
    rgb: String,
    rgba: String,
    hsl: String,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    h: u16,
    s: u8,
    l: u8,
}

impl From<&gizza_ai_image_average_color_core::MeanColor> for Mean {
    fn from(m: &gizza_ai_image_average_color_core::MeanColor) -> Self {
        Mean {
            hex: m.hex.clone(),
            hex_rgba: m.hex_rgba.clone(),
            rgb: m.rgb.clone(),
            rgba: m.rgba.clone(),
            hsl: m.hsl.clone(),
            r: m.r,
            g: m.g,
            b: m.b,
            a: m.a,
            h: m.h,
            s: m.s,
            l: m.l,
        }
    }
}

#[derive(Serialize)]
struct Resp {
    width: u32,
    height: u32,
    /// Pixels that contributed to the mean.
    pixels_counted: u64,
    /// Naive per-channel arithmetic mean in sRGB (what most tools report).
    simple: Mean,
    /// Perceptually correct mean taken in linear light — the recommended value.
    gamma_correct: Mean,
    /// Perceived brightness of the gamma-correct mean, 0-100.
    brightness: u8,
    /// True when the mean is dark (use white text/UI on top of it).
    is_dark: bool,
    /// `#rrggbb` complementary of the gamma-correct mean.
    complementary_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::boolean("ignore_transparency").default(true).describe(
            "Exclude (near-)transparent pixels from the average so a transparent PNG \
             background doesn't drag the mean toward black. Optional — defaults to true; \
             set false to fold every pixel in.",
        ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageAverageColor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-average-color",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the single mean color of an image (with a gamma-correct variant)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Compute the single average (mean) color of an image. Returns two means: 'simple', the naive per-channel arithmetic mean of the sRGB values (what most average-color tools report), and 'gamma_correct', the perceptually correct mean taken in linear light (sRGB decoded to linear, averaged, then re-encoded) — the recommended value. Each mean is given as #rrggbb hex, #rrggbbaa hex, rgb()/rgba()/hsl() strings, the r/g/b/a channels (0-255) and h/s/l values. Also returns the image dimensions, how many pixels were counted, the perceived brightness (0-100) and an is_dark flag for the gamma-correct mean (use white text on a dark mean), and its complementary hex color. By default (near-)transparent pixels are ignored; set ignore_transparency=false to fold every pixel in. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageAverageColor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-average-color")?;
    let ignore_transparency = args.ignore_transparency.unwrap_or(true);
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let avg = gizza_ai_image_average_color_core::average(&bytes, ignore_transparency)
        .map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        width: avg.width,
        height: avg.height,
        pixels_counted: avg.pixels_counted,
        simple: (&avg.simple).into(),
        gamma_correct: (&avg.gamma_correct).into(),
        brightness: avg.brightness,
        is_dark: avg.is_dark,
        complementary_hex: avg.complementary_hex,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize image-average-color response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "ignore_transparency": { "type": "boolean", "default": true, "description": "Exclude (near-)transparent pixels from the average so a transparent PNG background doesn't drag the mean toward black. Optional — defaults to true; set false to fold every pixel in." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
