//! gizza-ai/background-color-detector — detect an image's likely background colour.
//!
//! URL/ref image analyser: the descriptor single-sources the chat schema and CLI;
//! the handler resolves an image, runs the pure Rust core, and returns a flat JSON report.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_background_color_detector_core::{detect, Detection, Region};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_region")]
    region: String,
    #[serde(default = "d_border_percent")]
    border_percent: f64,
    #[serde(default = "d_tolerance")]
    tolerance: f64,
    #[serde(default = "d_uniform_threshold")]
    uniform_threshold: f64,
    #[serde(default = "d_ignore_transparency")]
    ignore_transparency: bool,
}

fn d_region() -> String {
    "border".into()
}
fn d_border_percent() -> f64 {
    10.0
}
fn d_tolerance() -> f64 {
    6.0
}
fn d_uniform_threshold() -> f64 {
    90.0
}
fn d_ignore_transparency() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    width: u32,
    height: u32,
    region: String,
    band_px: u32,
    stride: u32,
    sampled_pixels: u64,
    opaque_pixels: u64,
    transparent_pixels: u64,
    transparent_percent: f64,
    is_transparent: bool,
    hex: String,
    hex_rgba: String,
    rgb: String,
    rgba: String,
    hsl: String,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    color_name: String,
    coverage_percent: f64,
    is_uniform: bool,
    confidence: f64,
    second_hex: Option<String>,
    second_coverage_percent: f64,
    corner_top_left: String,
    corner_top_right: String,
    corner_bottom_left: String,
    corner_bottom_right: String,
    corners_agree: bool,
    max_corner_distance_percent: f64,
    luminance: f64,
    is_dark: bool,
    suggested_text_color: String,
    contrast_ratio: f64,
    warnings: Vec<String>,
    note: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("region", ["border", "corners", "edges", "full"])
                .default("border")
                .describe("Which part of the image votes for the background: border (default, all sides), corners (only corner patches), edges (edge strips without corners), or full (dominant colour of the whole image)."),
        )
        .param(
            Param::number("border_percent")
                .min(1.0)
                .max(50.0)
                .default(10.0)
                .describe("Thickness of the sampled border/corner band as a percent of the shorter image side, 1-50; ignored for region=full (default 10)."),
        )
        .param(
            Param::number("tolerance")
                .min(0.0)
                .max(100.0)
                .default(6.0)
                .describe("Per-channel colour distance, as percent of 0-255, still counted as the same background colour when computing coverage (default 6)."),
        )
        .param(
            Param::number("uniform_threshold")
                .min(0.0)
                .max(100.0)
                .default(90.0)
                .describe("Coverage percent required before the sampled region counts as a single flat background (default 90)."),
        )
        .param(
            Param::boolean("ignore_transparency")
                .default(true)
                .describe("When true, near-transparent pixels are excluded from the colour vote and a mostly transparent border can report is_transparent=true (default true)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/background-color-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect an image's likely background color from its border or corners.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Determine the likely background colour of an image by sampling the border, corners, edges or full image. Provide the image as url or ref. Returns hex, rgba, rgb, hsl, nearest colour name, background coverage, uniform/gradient verdict, confidence, per-corner hexes, transparency information, dark/light flag, suggested readable text colour and WCAG contrast ratio. Parameters: region=border|corners|edges|full (default border), border_percent 1-50 (default 10), tolerance 0-100 percent channel distance (default 6), uniform_threshold 0-100 (default 90), ignore_transparency boolean (default true). This is an analyser only; it does not remove or replace the background.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("background-color-detector")?;
    let region = Region::parse(&args.region).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let detection = detect(
        &bytes,
        region,
        args.border_percent,
        args.tolerance,
        args.uniform_threshold,
        args.ignore_transparency,
    )
    .map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&response(detection)).map_err(|e| {
        SkillError::Serialize(format!("serialize background-color-detector response: {e}"))
    })
}

fn note(d: &Detection) -> String {
    if d.is_transparent {
        return format!(
            "The sampled {} is transparent ({:.2}% transparent); underlying RGB is {}. Confidence {:.3}.",
            d.region, d.transparent_percent, d.hex, d.confidence
        );
    }
    let verdict = if d.is_uniform {
        "uniform"
    } else {
        "not uniform"
    };
    let second = d
        .second_hex
        .as_ref()
        .map(|s| format!(" Runner-up {s} covers {:.2}%.", d.second_coverage_percent))
        .unwrap_or_default();
    format!(
        "Likely background is {} ({}, {}) from the sampled {}; it is {} with {:.2}% coverage and confidence {:.3}. Use {} text on it (contrast {:.2}:1).{}",
        d.hex, d.rgb, d.color_name, d.region, verdict, d.coverage_percent, d.confidence, d.suggested_text_color, d.contrast_ratio, second
    )
}

fn response(d: Detection) -> Resp {
    let note = note(&d);
    Resp {
        width: d.width,
        height: d.height,
        region: d.region.to_string(),
        band_px: d.band_px,
        stride: d.stride,
        sampled_pixels: d.sampled_pixels,
        opaque_pixels: d.opaque_pixels,
        transparent_pixels: d.transparent_pixels,
        transparent_percent: d.transparent_percent,
        is_transparent: d.is_transparent,
        hex: d.hex,
        hex_rgba: d.hex_rgba,
        rgb: d.rgb,
        rgba: d.rgba,
        hsl: d.hsl,
        r: d.r,
        g: d.g,
        b: d.b,
        a: d.a,
        color_name: d.color_name.to_string(),
        coverage_percent: d.coverage_percent,
        is_uniform: d.is_uniform,
        confidence: d.confidence,
        second_hex: d.second_hex,
        second_coverage_percent: d.second_coverage_percent,
        corner_top_left: d.corner_top_left,
        corner_top_right: d.corner_top_right,
        corner_bottom_left: d.corner_bottom_left,
        corner_bottom_right: d.corner_bottom_right,
        corners_agree: d.corners_agree,
        max_corner_distance_percent: d.max_corner_distance_percent,
        luminance: d.luminance,
        is_dark: d.is_dark,
        suggested_text_color: d.suggested_text_color,
        contrast_ratio: d.contrast_ratio,
        warnings: d.warnings,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(uniform: bool) -> Detection {
        Detection {
            width: 40,
            height: 30,
            region: "border",
            band_px: 3,
            stride: 1,
            sampled_pixels: 396,
            opaque_pixels: 396,
            transparent_pixels: 0,
            transparent_percent: 0.0,
            is_transparent: false,
            hex: "#ffffff".into(),
            hex_rgba: "#ffffffff".into(),
            rgb: "rgb(255, 255, 255)".into(),
            rgba: "rgba(255, 255, 255, 1.00)".into(),
            hsl: "hsl(0, 0%, 100%)".into(),
            r: 255,
            g: 255,
            b: 255,
            a: 255,
            color_name: "white",
            coverage_percent: if uniform { 100.0 } else { 45.0 },
            is_uniform: uniform,
            confidence: if uniform { 1.0 } else { 0.46 },
            second_hex: if uniform {
                None
            } else {
                Some("#000000".into())
            },
            second_coverage_percent: if uniform { 0.0 } else { 40.0 },
            corner_top_left: "#ffffff".into(),
            corner_top_right: "#ffffff".into(),
            corner_bottom_left: "#ffffff".into(),
            corner_bottom_right: "#ffffff".into(),
            corners_agree: true,
            max_corner_distance_percent: 0.0,
            luminance: 1.0,
            is_dark: false,
            suggested_text_color: "#000000".into(),
            contrast_ratio: 21.0,
            warnings: vec![],
        }
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_region(), "border");
        assert_eq!(d_border_percent(), 10.0);
        assert_eq!(d_tolerance(), 6.0);
        assert_eq!(d_uniform_threshold(), 90.0);
        assert!(d_ignore_transparency());
        assert_eq!(Region::parse("edges").unwrap(), Region::Edges);
    }

    #[test]
    fn args_parse_from_bare_url_using_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://example.com/bg.png"}"#).unwrap();
        assert_eq!(a.region, "border");
        assert_eq!(a.border_percent, 10.0);
        assert_eq!(a.tolerance, 6.0);
        assert_eq!(a.uniform_threshold, 90.0);
        assert!(a.ignore_transparency);
    }

    #[test]
    fn note_summarizes_uniform_and_non_uniform_backgrounds() {
        let uniform = response(sample(true));
        assert!(
            uniform.note.contains("Likely background is #ffffff"),
            "{}",
            uniform.note
        );
        assert!(uniform.note.contains("uniform"), "{}", uniform.note);
        assert_eq!(uniform.suggested_text_color, "#000000");
        let mixed = response(sample(false));
        assert!(mixed.note.contains("not uniform"), "{}", mixed.note);
        assert!(mixed.note.contains("Runner-up #000000"), "{}", mixed.note);
    }

    #[test]
    fn transparent_note_is_explicit() {
        let mut d = sample(true);
        d.is_transparent = true;
        d.transparent_percent = 100.0;
        let resp = response(d);
        assert!(resp.note.contains("transparent"), "{}", resp.note);
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "region": { "type": "string", "enum": ["border", "corners", "edges", "full"], "default": "border", "description": "Which part of the image votes for the background: border (default, all sides), corners (only corner patches), edges (edge strips without corners), or full (dominant colour of the whole image)." },
                    "border_percent": { "type": "number", "minimum": 1, "maximum": 50, "default": 10.0, "description": "Thickness of the sampled border/corner band as a percent of the shorter image side, 1-50; ignored for region=full (default 10)." },
                    "tolerance": { "type": "number", "minimum": 0, "maximum": 100, "default": 6.0, "description": "Per-channel colour distance, as percent of 0-255, still counted as the same background colour when computing coverage (default 6)." },
                    "uniform_threshold": { "type": "number", "minimum": 0, "maximum": 100, "default": 90.0, "description": "Coverage percent required before the sampled region counts as a single flat background (default 90)." },
                    "ignore_transparency": { "type": "boolean", "default": true, "description": "When true, near-transparent pixels are excluded from the colour vote and a mostly transparent border can report is_transparent=true (default true)." }
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
