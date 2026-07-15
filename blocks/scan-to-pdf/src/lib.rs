//! gizza-ai/scan-to-pdf — turn one or more phone photos of documents into a
//! cleaned, deskewed, high-contrast multi-page PDF scan.
//!
//! Pipeline: resolve each image source via `block-utils` `resolve_source`
//! (URL fetch or attachment ref) → pure `core::scan_to_pdf` (per-photo downscale,
//! 90° rotate, auto-deskew, enhancement mode, then one PDF page each) → base64
//! PDF envelope. `Input::None` + a required `images` source_list (like
//! images-to-pdf — sources arrive as an array param, not a single media input).
//! Surfaces: chat + CLI. No standalone page (array input + pure-Rust PDF bytes,
//! like every `*-to-pdf` tool — the page driver has no PDF render mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_scan_to_pdf_core::{scan_to_pdf, Mode, PageSize, ScanOptions};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    images: Vec<SourceFields>,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_deskew")]
    deskew: bool,
    #[serde(default = "default_rotate")]
    rotate: String,
    #[serde(default = "default_contrast")]
    contrast: f64,
    #[serde(default = "default_brightness")]
    brightness: f64,
    #[serde(default = "default_page_size")]
    page_size: String,
}
fn default_mode() -> String {
    "magic".to_string()
}
fn default_deskew() -> bool {
    true
}
fn default_rotate() -> String {
    "0".to_string()
}
fn default_contrast() -> f64 {
    1.0
}
fn default_brightness() -> f64 {
    0.0
}
fn default_page_size() -> String {
    "fit".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("images", 1)
                .required()
                .describe("Ordered list of document-photo sources (PNG/JPEG/WebP/GIF/BMP). Each item has exactly one of `url` or `ref`. One cleaned page per photo, in order."),
        )
        .param(
            Param::enumv("mode", ["magic", "grayscale", "blackwhite", "color"])
                .default("magic")
                .describe("Enhancement filter. magic (default): whiten the paper, lift contrast + saturation for the everyday colour 'office scan'. grayscale: perception-weighted grey. blackwhite: adaptive local-mean threshold → crisp pure black-on-white, despeckled, for forms/contracts. color: keep colour, only apply brightness/contrast."),
        )
        .param(
            Param::boolean("deskew")
                .default(true)
                .describe("Auto-straighten a small tilt (up to ±12°) by detecting the skew angle and rotating to undo it. Default true. Turn off to keep the photo's original angle."),
        )
        .param(
            Param::enumv("rotate", ["0", "90", "180", "270"])
                .default("0")
                .describe("Manual clockwise rotation in degrees for phone orientation, applied before deskew: 0 (default), 90, 180 or 270."),
        )
        .param(
            Param::number("contrast")
                .min(0.5)
                .max(3.0)
                .default(1.0)
                .describe("Contrast multiplier around mid-grey (0.5–3.0, 1.0 = none). Applies in magic/grayscale/color modes; higher = punchier text. Default 1.0."),
        )
        .param(
            Param::number("brightness")
                .min(-100.0)
                .max(100.0)
                .default(0.0)
                .describe("Brightness offset from -100 to 100 (0 = none). Positive brightens; in blackwhite mode it also keeps less ink (whiter background). Default 0."),
        )
        .param(
            Param::enumv("page_size", ["fit", "a4", "letter"])
                .default("fit")
                .describe("Output page size. fit (default): one page sized exactly to each photo. a4 / letter: scale each photo to fit a centred A4 (595×842 pt) or US Letter (612×792 pt) page."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Map the parsed args into the core `ScanOptions`, validating enum/rotate values.
fn options_from(args: &Args) -> Result<ScanOptions, SkillError> {
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let page_size = PageSize::parse(&args.page_size).map_err(SkillError::InvalidArgs)?;
    let rotate: u16 = args
        .rotate
        .parse()
        .ok()
        .filter(|d| matches!(d, 0 | 90 | 180 | 270))
        .ok_or_else(|| {
            SkillError::InvalidArgs(format!(
                "rotate must be 0, 90, 180 or 270 (got `{}`)",
                args.rotate
            ))
        })?;
    Ok(ScanOptions {
        mode,
        deskew: args.deskew,
        rotate,
        contrast: args.contrast as f32,
        brightness: args.brightness as f32,
        page_size,
    })
}

#[cfg(target_arch = "wasm32")]
struct ScanToPdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/scan-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn document photos into a cleaned, deskewed, high-contrast PDF scan.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Turn one or more phone photos of documents into a cleaned, deskewed, high-contrast multi-page PDF scan (one page per photo, in order). Each photo is a URL or a `ref` to an uploaded image (PNG/JPEG/WebP/GIF/BMP). Choose an enhancement mode (magic colour, grayscale, black & white, or plain colour), auto-straighten a small tilt, rotate for orientation, tune brightness/contrast, and pick the output page size (fit/A4/Letter). Note: does NOT do 4-corner auto-crop/perspective de-warp or OCR/searchable text.",
        parameters = schema_json()
    ),
)]
impl ScanToPdf {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("scan-to-pdf")?;
    if args.images.is_empty() {
        return Err(SkillError::InvalidArgs("scan-to-pdf needs at least 1 image".into()));
    }
    let opts = options_from(&args)?;

    let mut imgs: Vec<Vec<u8>> = Vec::with_capacity(args.images.len());
    for field in args.images {
        let (bytes, _mime, _name) =
            resolve_source(field.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
        imgs.push(bytes);
    }
    let n = imgs.len();

    let pdf = scan_to_pdf(&imgs, &opts).map_err(SkillError::InvalidArgs)?;
    if pdf.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output PDF is {} bytes, over the {MAX_OUTPUT_BYTES} cap",
            pdf.len()
        )));
    }
    let out_len = pdf.len();
    let encoded = B64.encode(&pdf);
    let data_url = format!("data:application/pdf;base64,{encoded}");

    let env = Envelope {
        for_llm: format!(
            "scanned {n} photo(s) into a {out_len}-byte {} PDF (scan.pdf)",
            opts.mode.label()
        ),
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename: "scan.pdf".to_string(),
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema exactly (any LLM-facing drift fails here).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "images": {
                        "type": "array",
                        "minItems": 1,
                        "description": "Ordered list of document-photo sources (PNG/JPEG/WebP/GIF/BMP). Each item has exactly one of `url` or `ref`. One cleaned page per photo, in order.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["magic", "grayscale", "blackwhite", "color"],
                        "default": "magic",
                        "description": "Enhancement filter. magic (default): whiten the paper, lift contrast + saturation for the everyday colour 'office scan'. grayscale: perception-weighted grey. blackwhite: adaptive local-mean threshold → crisp pure black-on-white, despeckled, for forms/contracts. color: keep colour, only apply brightness/contrast."
                    },
                    "deskew": {
                        "type": "boolean",
                        "default": true,
                        "description": "Auto-straighten a small tilt (up to ±12°) by detecting the skew angle and rotating to undo it. Default true. Turn off to keep the photo's original angle."
                    },
                    "rotate": {
                        "type": "string",
                        "enum": ["0", "90", "180", "270"],
                        "default": "0",
                        "description": "Manual clockwise rotation in degrees for phone orientation, applied before deskew: 0 (default), 90, 180 or 270."
                    },
                    "contrast": {
                        "type": "number",
                        "minimum": 0.5,
                        "maximum": 3,
                        "default": 1.0,
                        "description": "Contrast multiplier around mid-grey (0.5–3.0, 1.0 = none). Applies in magic/grayscale/color modes; higher = punchier text. Default 1.0."
                    },
                    "brightness": {
                        "type": "number",
                        "minimum": -100,
                        "maximum": 100,
                        "default": 0.0,
                        "description": "Brightness offset from -100 to 100 (0 = none). Positive brightens; in blackwhite mode it also keeps less ink (whiter background). Default 0."
                    },
                    "page_size": {
                        "type": "string",
                        "enum": ["fit", "a4", "letter"],
                        "default": "fit",
                        "description": "Output page size. fit (default): one page sized exactly to each photo. a4 / letter: scale each photo to fit a centred A4 (595×842 pt) or US Letter (612×792 pt) page."
                    }
                },
                "required": ["images"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_defaults() {
        let a: Args = serde_json::from_str(r#"{"images":[{"url":"https://x/a.png"}]}"#).unwrap();
        assert_eq!(a.images.len(), 1);
        let o = options_from(&a).unwrap();
        assert_eq!(o.mode, Mode::Magic);
        assert!(o.deskew);
        assert_eq!(o.rotate, 0);
        assert_eq!(o.page_size, PageSize::Fit);
    }

    #[test]
    fn args_parse_overrides() {
        let a: Args = serde_json::from_str(
            r#"{"images":[{"ref":"c_1"},{"url":"https://x/b.jpg"}],"mode":"blackwhite","deskew":false,"rotate":"90","contrast":1.5,"brightness":20,"page_size":"a4"}"#,
        )
        .unwrap();
        assert_eq!(a.images.len(), 2);
        let o = options_from(&a).unwrap();
        assert_eq!(o.mode, Mode::BlackWhite);
        assert!(!o.deskew);
        assert_eq!(o.rotate, 90);
        assert_eq!(o.page_size, PageSize::A4);
        assert_eq!(o.contrast, 1.5);
        assert_eq!(o.brightness, 20.0);
    }

    #[test]
    fn rejects_bad_rotate() {
        let a: Args = serde_json::from_str(r#"{"images":[{"url":"https://x/a.png"}],"rotate":"45"}"#).unwrap();
        assert!(options_from(&a).is_err());
    }
}
