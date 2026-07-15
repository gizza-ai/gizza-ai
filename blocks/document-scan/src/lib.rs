//! gizza-ai/document-scan — detect a document's four corners in a photo and
//! perspective-correct (dewarp) it into a flat, cropped, tonally-cleaned PNG scan.
//! Pure-Rust (`image` crate) — runs on all backends incl. the chat SW. Surfaces:
//! chat + CLI (image input + image bytes output → no page, like blur-image /
//! scan-to-pdf).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_document_scan_core::{document_scan, Mode, Output, Quad, ScanOptions};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    corners: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_rotate")]
    rotate: String,
    #[serde(default)]
    margin: f64,
}
fn default_mode() -> String {
    "magic".to_string()
}
fn default_output() -> String {
    "auto".to_string()
}
fn default_rotate() -> String {
    "0".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("corners")
                .describe("Optional manual page corners as 8 comma-separated source pixels `x0,y0,x1,y1,x2,y2,x3,y3` in order top-left, top-right, bottom-right, bottom-left. Leave empty to auto-detect the page (works best when the page is lighter than its background and fully in frame)."),
        )
        .param(
            Param::enumv("mode", ["magic", "grayscale", "blackwhite", "color"])
                .default("magic")
                .describe("Tonal cleanup of the flattened page. magic (default): whiten the paper and lift contrast while keeping colour (the everyday 'scan' look). grayscale: perception-weighted grey. blackwhite: Otsu threshold → crisp pure black-on-white for forms/contracts. color: keep the warped colours unchanged."),
        )
        .param(
            Param::enumv("output", ["auto", "a4", "letter", "square"])
                .default("auto")
                .describe("Output proportions. auto (default): the page's own measured shape. a4 / letter: force ISO A4 (210:297) or US Letter (8.5:11), matching the page's portrait/landscape orientation. square: a square."),
        )
        .param(
            Param::enumv("rotate", ["0", "90", "180", "270"])
                .default("0")
                .describe("Clockwise rotation of the finished scan in degrees, to fix orientation: 0 (default), 90, 180 or 270."),
        )
        .param(
            Param::number("margin")
                .min(0.0)
                .max(25.0)
                .default(0.0)
                .describe("White border added around the scan, as a percent of its longer side (0-25, default 0)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Parse the optional `corners` string into a quad (TL, TR, BR, BL).
fn parse_corners(s: &str) -> Result<Option<Quad>, SkillError> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let nums: Vec<f64> = t
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            SkillError::InvalidArgs(
                "corners must be 8 comma-separated numbers `x0,y0,x1,y1,x2,y2,x3,y3`".into(),
            )
        })?;
    if nums.len() != 8 {
        return Err(SkillError::InvalidArgs(format!(
            "corners needs exactly 8 numbers (x0,y0,...,x3,y3); got {}",
            nums.len()
        )));
    }
    Ok(Some([
        (nums[0], nums[1]),
        (nums[2], nums[3]),
        (nums[4], nums[5]),
        (nums[6], nums[7]),
    ]))
}

/// Map the parsed args into core `ScanOptions`, validating enum values.
fn options_from(args: &Args) -> Result<ScanOptions, SkillError> {
    let mode = Mode::parse(&args.mode).map_err(SkillError::InvalidArgs)?;
    let output = Output::parse(&args.output).map_err(SkillError::InvalidArgs)?;
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
    if !args.margin.is_finite() || !(0.0..=25.0).contains(&args.margin) {
        return Err(SkillError::InvalidArgs(format!(
            "margin must be between 0 and 25 (got {})",
            args.margin
        )));
    }
    Ok(ScanOptions {
        corners: parse_corners(&args.corners)?,
        mode,
        output,
        rotate,
        margin: args.margin as f32,
    })
}

#[cfg(target_arch = "wasm32")]
struct DocumentScan;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/document-scan",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Perspective-correct a document photo into a flat, cropped scan.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Turn a photo of a document into a flat, cropped 'scan': detect the page's four corners and perspective-correct (dewarp) it to a rectangle, then clean it up tonally. Provide the image as either url (HTTP/HTTPS) or ref. Leave corners empty to auto-detect the page (classical contrast detection — works best when the page is lighter than its background and fully in frame; on cluttered/low-contrast photos it errors and you pass corners `x0,y0,...,x3,y3`). Pick an enhancement mode (magic colour / grayscale / black & white / colour), output proportions (auto/A4/Letter/square), a rotation and a white margin. Returns a PNG. Note: does NOT do OCR/searchable text, and auto-detect is not ML — pass explicit corners for tricky scenes.",
        parameters = schema_json()
    ),
)]
impl DocumentScan {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("document-scan")?;
    let opts = options_from(&args)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = document_scan(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "scan.png".to_string(),
        format!(
            "perspective-corrected {} scan ({} bytes PNG)",
            opts.mode.label(),
            png.len()
        ),
        MAX_OUTPUT_BYTES,
    )
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
                    "url":     { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":     { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "corners": { "type": "string", "description": "Optional manual page corners as 8 comma-separated source pixels `x0,y0,x1,y1,x2,y2,x3,y3` in order top-left, top-right, bottom-right, bottom-left. Leave empty to auto-detect the page (works best when the page is lighter than its background and fully in frame)." },
                    "mode": {
                        "type": "string",
                        "enum": ["magic", "grayscale", "blackwhite", "color"],
                        "default": "magic",
                        "description": "Tonal cleanup of the flattened page. magic (default): whiten the paper and lift contrast while keeping colour (the everyday 'scan' look). grayscale: perception-weighted grey. blackwhite: Otsu threshold → crisp pure black-on-white for forms/contracts. color: keep the warped colours unchanged."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["auto", "a4", "letter", "square"],
                        "default": "auto",
                        "description": "Output proportions. auto (default): the page's own measured shape. a4 / letter: force ISO A4 (210:297) or US Letter (8.5:11), matching the page's portrait/landscape orientation. square: a square."
                    },
                    "rotate": {
                        "type": "string",
                        "enum": ["0", "90", "180", "270"],
                        "default": "0",
                        "description": "Clockwise rotation of the finished scan in degrees, to fix orientation: 0 (default), 90, 180 or 270."
                    },
                    "margin": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 25,
                        "default": 0.0,
                        "description": "White border added around the scan, as a percent of its longer side (0-25, default 0)."
                    }
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
    fn parse_corners_ok_and_errors() {
        let q = parse_corners("10,20,30,40,50,60,70,80").unwrap().unwrap();
        assert_eq!(q[0], (10.0, 20.0));
        assert_eq!(q[3], (70.0, 80.0));
        assert!(parse_corners("").unwrap().is_none());
        assert!(parse_corners("1,2,3").is_err());
        assert!(parse_corners("a,b,c,d,e,f,g,h").is_err());
    }

    #[test]
    fn options_defaults_and_overrides() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/a.jpg"}"#).unwrap();
        let o = options_from(&a).unwrap();
        assert_eq!(o.mode, Mode::Magic);
        assert_eq!(o.output, Output::Auto);
        assert_eq!(o.rotate, 0);
        assert!(o.corners.is_none());

        let b: Args = serde_json::from_str(
            r#"{"ref":"c_1","mode":"blackwhite","output":"a4","rotate":"90","margin":5,"corners":"1,2,3,4,5,6,7,8"}"#,
        )
        .unwrap();
        let o = options_from(&b).unwrap();
        assert_eq!(o.mode, Mode::BlackWhite);
        assert_eq!(o.output, Output::A4);
        assert_eq!(o.rotate, 90);
        assert_eq!(o.margin, 5.0);
        assert!(o.corners.is_some());
    }

    #[test]
    fn options_reject_bad_values() {
        let bad_rot: Args =
            serde_json::from_str(r#"{"url":"https://x/a.jpg","rotate":"45"}"#).unwrap();
        assert!(options_from(&bad_rot).is_err());
        let bad_margin: Args =
            serde_json::from_str(r#"{"url":"https://x/a.jpg","margin":80}"#).unwrap();
        assert!(options_from(&bad_margin).is_err());
        let bad_mode: Args =
            serde_json::from_str(r#"{"url":"https://x/a.jpg","mode":"sepia"}"#).unwrap();
        assert!(options_from(&bad_mode).is_err());
    }
}
