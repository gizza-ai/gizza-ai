//! gizza-ai/chromatic-aberration-fix — reduce purple/green lens fringing on an
//! image and return corrected image bytes. Pure Rust → chat + CLI; no standalone
//! page because the current generator has no binary-image-output page surface.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_chromatic_aberration_fix_core::{fix_chromatic_aberration, Format, Report, Settings};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_purple_amount")]
    purple_amount: u32,
    #[serde(default = "default_green_amount")]
    green_amount: u32,
    #[serde(default = "default_edge_threshold")]
    edge_threshold: u32,
    #[serde(default = "default_radius")]
    radius: u32,
    #[serde(default = "default_hue_tolerance")]
    hue_tolerance: u32,
    #[serde(default)]
    red_shift: f32,
    #[serde(default)]
    blue_shift: f32,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_quality")]
    quality: u32,
}

fn default_purple_amount() -> u32 {
    8
}
fn default_green_amount() -> u32 {
    5
}
fn default_edge_threshold() -> u32 {
    20
}
fn default_radius() -> u32 {
    4
}
fn default_hue_tolerance() -> u32 {
    40
}
fn default_format() -> String {
    "auto".into()
}
fn default_quality() -> u32 {
    90
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::integer("purple_amount").min(0.0).max(20.0).default(8).describe("Strength for reducing purple/violet halos around high-contrast edges (0-20, default 8). 0 disables the purple pass; 20 pulls red/blue channel excess fully down to green."))
        .param(Param::integer("green_amount").min(0.0).max(20.0).default(5).describe("Strength for reducing green halos around high-contrast edges (0-20, default 5). 0 disables the green pass; 20 pulls green channel excess fully down to the brighter red/blue reference."))
        .param(Param::integer("edge_threshold").min(0.0).max(255.0).default(20).describe("Minimum local luma contrast (0-255, default 20) before a pixel is considered near an edge. Raise to protect low-contrast colored subjects; 0 allows correction everywhere."))
        .param(Param::integer("radius").min(1.0).max(20.0).default(4).describe("How many pixels the edge mask reaches around a detected edge (1-20, default 4). Larger values catch wider halos but can affect nearby purple/green detail."))
        .param(Param::integer("hue_tolerance").min(5.0).max(90.0).default(40).describe("Hue half-window in degrees around purple (285°) and green (120°), 5-90, default 40. Wider tolerances catch more fringe colors but risk desaturating real subject color."))
        .param(Param::number("red_shift").min(-10.0).max(10.0).default(0.0).describe("Optional radial lateral chromatic-aberration correction for the red channel, measured in pixels at the image corner (-10 to 10, default 0). Positive pulls red inward; negative pushes it outward."))
        .param(Param::number("blue_shift").min(-10.0).max(10.0).default(0.0).describe("Optional radial lateral chromatic-aberration correction for the blue channel, measured in pixels at the image corner (-10 to 10, default 0). Positive pulls blue inward; negative pushes it outward."))
        .param(Param::enumv("format", ["auto", "png", "jpeg", "webp"]).default("auto").describe("Output image format: auto keeps JPEG/WebP input as that format and uses PNG otherwise; png preserves alpha; jpeg flattens alpha; webp writes WebP."))
        .param(Param::integer("quality").min(1.0).max(100.0).default(90).describe("Encoder quality for JPEG output (1-100, default 90). PNG/WebP are written with the image crate's lossless/default encoder and ignore this value."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ChromaticAberrationFix;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chromatic-aberration-fix",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reduce purple and green chromatic-aberration fringes in an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Reduce purple/violet and green chromatic-aberration halos on high-contrast image edges. The tool first optionally applies radial red/blue lateral channel correction, then performs edge-gated defringe passes that only reduce channel excess instead of bluntly desaturating the whole image. Provide an image as url (HTTP/HTTPS) or ref. Use amount/radius/hue controls carefully: strong settings can gray out legitimate purple or green subject detail near edges.",
        parameters = schema_json()
    ),
)]
impl ChromaticAberrationFix {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("chromatic-aberration-fix")?;
    let (input_bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let format = Format::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let settings = Settings {
        purple_amount: args.purple_amount,
        green_amount: args.green_amount,
        edge_threshold: args.edge_threshold,
        radius: args.radius,
        hue_tolerance: args.hue_tolerance,
        red_shift: args.red_shift,
        blue_shift: args.blue_shift,
        format,
        quality: args.quality,
    };
    let (output, report) =
        fix_chromatic_aberration(&input_bytes, settings).map_err(SkillError::InvalidArgs)?;
    let filename = filename_with_suffix(&in_filename, "-defringed", report.format.extension());
    let summary = summary(&in_filename, &report, output.len());
    build_media_envelope(
        &output,
        report.format.mime(),
        filename,
        summary,
        MAX_OUTPUT_BYTES,
    )
}

fn summary(source: &str, report: &Report, output_bytes: usize) -> String {
    let lateral = if report.lateral_applied {
        "; lateral red/blue shift applied"
    } else {
        ""
    };
    format!(
        "corrected {source}: {}x{}, {} purple-fringe pixel(s), {} green-fringe pixel(s){lateral}; wrote {} bytes {}",
        report.width,
        report.height,
        report.purple_pixels,
        report.green_pixels,
        output_bytes,
        report.format.extension()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived.get("properties").unwrap();
        assert!(props.get("url").is_some());
        assert!(props.get("ref").is_some());
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["auto", "png", "jpeg", "webp"])
        );
        assert_eq!(props["purple_amount"]["default"], 8);
        assert_eq!(props["green_amount"]["default"], 5);
        assert_eq!(props["radius"]["maximum"], 20);
        assert_eq!(derived["additionalProperties"], false);
        assert!(
            derived.get("oneOf").is_some(),
            "image input must require url or ref"
        );
    }

    #[test]
    fn summary_reports_counts_and_extension() {
        let report = Report {
            width: 8,
            height: 4,
            purple_pixels: 4,
            green_pixels: 0,
            lateral_applied: true,
            format: Format::Png,
        };
        let s = summary("edge.png", &report, 1234);
        assert!(s.contains("edge.png"));
        assert!(s.contains("8x4"));
        assert!(s.contains("4 purple"));
        assert!(s.contains("lateral"));
        assert!(s.contains("1234 bytes png"));
    }

    #[test]
    fn defringed_filename_keeps_basename_suffix() {
        assert_eq!(
            filename_with_suffix("photo.jpg", "-defringed", "png"),
            "photo-defringed.png"
        );
    }
}
