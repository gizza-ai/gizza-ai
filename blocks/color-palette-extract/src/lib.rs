//! gizza-ai/color-palette-extract — extract the dominant color palette from an
//! image as hex codes.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::extract` (decode +
//! NeuQuant palette + dominance ordering) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report — the F3 no-page
//! file-input pattern, like image-info / image-color-picker report tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_COLORS: i64 = 6;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Requested palette size (1..=64). Defaults to 6.
    #[serde(default)]
    colors: Option<i64>,
}

#[derive(Serialize)]
struct Swatch {
    hex: String,
    rgb: String,
    /// CSS `hsl(H, S%, L%)` string.
    hsl: String,
    r: u8,
    g: u8,
    b: u8,
    /// Hue 0-360 (degrees), rounded.
    h: u16,
    /// Saturation 0-100 (%), rounded.
    s: u8,
    /// Lightness 0-100 (%), rounded.
    l: u8,
    /// Share of (opaque) pixels, 0.0..=1.0.
    fraction: f64,
    /// Share of (opaque) pixels as a "12.3%" string.
    percent: String,
}

/// RGB (0-255) → HSL with H in 0..360 deg, S/L in 0..100 %.
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d.abs() < f64::EPSILON {
        (0.0, 0.0)
    } else {
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if (max - rf).abs() < f64::EPSILON {
            ((gf - bf) / d).rem_euclid(6.0)
        } else if (max - gf).abs() < f64::EPSILON {
            (bf - rf) / d + 2.0
        } else {
            (rf - gf) / d + 4.0
        };
        (h * 60.0, s)
    };
    (
        h.round() as u16 % 360,
        (s * 100.0).round() as u8,
        (l * 100.0).round() as u8,
    )
}

#[derive(Serialize)]
struct Resp {
    width: u32,
    height: u32,
    /// Number of colors actually returned (<= requested).
    color_count: usize,
    /// Dominant colors, most-dominant first.
    colors: Vec<Swatch>,
    /// Convenience: just the hex codes, in dominance order.
    hex: Vec<String>,
    /// Ready-to-paste CSS custom properties, e.g.
    /// ":root {\n  --color-1: #ff0000;\n  ...\n}".
    css_variables: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::integer("colors")
            .min(1.0)
            .max(64.0)
            .default(DEFAULT_COLORS)
            .describe("How many dominant colors to extract (1-64). Optional — defaults to 6."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ColorPaletteExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-palette-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the dominant color palette from an image as hex codes",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract the dominant color palette from an image. For each color it returns the #rrggbb hex, rgb() and hsl() strings, the r/g/b channels (0-255) and h/s/l values, and its share of the image (fraction + percent), ordered most-dominant first. Also returns a plain list of the hex codes, a ready-to-paste CSS custom-properties block (:root { --color-1: ...; }), and the image dimensions. The palette is derived from the image itself (NeuQuant), and fully transparent pixels are ignored. Use the colors parameter (1-64, default 6) to set how many colors to extract. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ColorPaletteExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("color-palette-extract")?;
    let n = args.colors.unwrap_or(DEFAULT_COLORS).clamp(1, 64) as usize;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let p =
        gizza_ai_color_palette_extract_core::extract(&bytes, n).map_err(SkillError::InvalidArgs)?;

    let colors: Vec<Swatch> = p
        .colors
        .iter()
        .map(|s| {
            let (h, sat, l) = rgb_to_hsl(s.r, s.g, s.b);
            Swatch {
                hex: s.hex.clone(),
                rgb: format!("rgb({}, {}, {})", s.r, s.g, s.b),
                hsl: format!("hsl({h}, {sat}%, {l}%)"),
                r: s.r,
                g: s.g,
                b: s.b,
                h,
                s: sat,
                l,
                fraction: s.fraction,
                percent: format!("{:.1}%", s.fraction * 100.0),
            }
        })
        .collect();
    let hex: Vec<String> = p.colors.iter().map(|s| s.hex.clone()).collect();
    let mut css_variables = String::from(":root {\n");
    for (i, code) in hex.iter().enumerate() {
        css_variables.push_str(&format!("  --color-{}: {};\n", i + 1, code));
    }
    css_variables.push('}');

    let resp = Resp {
        width: p.width,
        height: p.height,
        color_count: colors.len(),
        colors,
        hex,
        css_variables,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize color-palette-extract response: {e}"))
    })
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "colors": { "type": "integer", "minimum": 1, "maximum": 64, "default": 6, "description": "How many dominant colors to extract (1-64). Optional — defaults to 6." }
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
    fn rgb_to_hsl_known_values() {
        assert_eq!(rgb_to_hsl(255, 0, 0), (0, 100, 50)); // pure red
        assert_eq!(rgb_to_hsl(0, 255, 0), (120, 100, 50)); // pure green
        assert_eq!(rgb_to_hsl(0, 0, 255), (240, 100, 50)); // pure blue
        assert_eq!(rgb_to_hsl(0, 0, 0), (0, 0, 0)); // black
        assert_eq!(rgb_to_hsl(255, 255, 255), (0, 0, 100)); // white
        assert_eq!(rgb_to_hsl(128, 128, 128), (0, 0, 50)); // gray
    }
}
