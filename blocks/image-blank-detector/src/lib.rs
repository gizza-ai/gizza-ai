//! gizza-ai/image-blank-detector — flag blank, solid-colour, all-white,
//! all-black and fully transparent images, the tell-tale output of a render,
//! screenshot or export that silently failed.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::detect` (decode +
//! whole-frame uniformity measurement) → flat JSON the LLM reads directly, with
//! a one-line `note` so a batch triage reads without post-processing.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report — the F3 no-page
//! file-input pattern, like image-info / image-average-color /
//! background-color-detector; the generator's pure-tool page can't hand uploaded
//! bytes to a wasm decoder).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_image_blank_detector_core::{detect, Detection, NEAR_BLANK, NOT_BLANK, TRANSPARENT};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_tolerance")]
    tolerance: f64,
    #[serde(default = "d_blank_threshold")]
    blank_threshold: f64,
    #[serde(default = "d_ignore_transparency")]
    ignore_transparency: bool,
}

fn d_tolerance() -> f64 {
    2.0
}
fn d_blank_threshold() -> f64 {
    99.5
}
fn d_ignore_transparency() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    width: u32,
    height: u32,
    total_pixels: u64,
    transparent_pixels: u64,
    transparent_percent: f64,
    has_alpha: bool,
    is_blank: bool,
    verdict: String,
    reason: String,
    confidence: f64,
    dominant_hex: String,
    dominant_hex_rgba: String,
    dominant_rgb: String,
    color_name: String,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    coverage_percent: f64,
    unique_colors: u64,
    unique_colors_capped: bool,
    mean_hex: String,
    luma_min: u8,
    luma_max: u8,
    luma_mean: f64,
    luma_stddev: f64,
    channel_range: u8,
    entropy: f64,
    warnings: Vec<String>,
    /// One-line human summary — what a batch triage prints per image.
    note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("tolerance")
                .min(0.0)
                .max(100.0)
                .default(2.0)
                .describe(
                    "How far a pixel may differ per channel and still count as the same colour, \
                     as a percent of the 0-255 range, 0-100. 0 demands exactly identical pixels; \
                     the default 2 (about 5 levels) absorbs JPEG/WebP compression noise on an \
                     otherwise flat fill (default 2).",
                ),
        )
        .param(
            Param::number("blank_threshold")
                .min(50.0)
                .max(100.0)
                .default(99.5)
                .describe(
                    "Percent of pixels that must match the dominant colour before the image is \
                     called blank, 50-100. 100 flags only perfectly uniform images; the default \
                     99.5 also flags a page that is empty apart from a small watermark (default \
                     99.5).",
                ),
        )
        .param(
            Param::boolean("ignore_transparency")
                .default(true)
                .describe(
                    "When true, near-transparent pixels are treated as one 'empty' colour so the \
                     junk RGB some encoders leave underneath alpha=0 cannot fake content; set \
                     false to compare the raw RGBA values as stored (default true).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-blank-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect blank, solid-color, all-white, all-black or fully transparent images.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Check whether an image is blank — an all-white or all-black frame, a single flat colour fill, a fully transparent canvas, or a frame that is empty apart from a stray watermark. These are the signatures of a render, screenshot, thumbnail or export that silently failed, so use this to triage a batch before the empty files ship. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). Every pixel is inspected — nothing is sub-sampled — and the answer comes with its evidence: is_blank, verdict (all_white, all_black, solid_color, transparent, near_blank or not_blank), a plain-English reason, confidence 0-1, the dominant fill colour as hex/rgb plus a colour name, the percent of pixels matching it, distinct colour count, transparent-pixel share, luma min/max/mean/standard deviation, per-channel range, and the Shannon entropy of the luma histogram (0 on a flat image, above 6 on a photo). Parameters: tolerance 0-100 percent channel distance (default 2), blank_threshold 50-100 percent coverage required (default 99.5), ignore_transparency boolean (default true). This is an analyser only; it does not modify or delete anything.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("image-blank-detector")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let detection = detect(
        &bytes,
        args.tolerance,
        args.blank_threshold,
        args.ignore_transparency,
    )
    .map_err(SkillError::InvalidArgs)?;
    let resp = response(detection, (!filename.is_empty()).then_some(filename));
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize image-blank-detector response: {e}")))
}

/// The one-line summary a batch triage prints per image: the answer first, the
/// number that decided it second.
fn note(d: &Detection) -> String {
    let head = match d.verdict {
        NOT_BLANK => "NOT BLANK".to_string(),
        TRANSPARENT => "BLANK (fully transparent)".to_string(),
        NEAR_BLANK => "BLANK (empty apart from a small mark)".to_string(),
        other => format!("BLANK ({})", other.replace('_', " ")),
    };
    format!(
        "{head} — {}x{}, dominant {} ({}) covers {:.2}% of the pixels, {} distinct colour(s), \
         entropy {:.2} bits. Confidence {:.3}.",
        d.width,
        d.height,
        d.dominant_hex,
        d.color_name,
        d.coverage_percent,
        if d.unique_colors_capped {
            format!("{}+", d.unique_colors)
        } else {
            d.unique_colors.to_string()
        },
        d.entropy,
        d.confidence
    )
}

fn response(d: Detection, filename: Option<String>) -> Resp {
    let note = note(&d);
    Resp {
        width: d.width,
        height: d.height,
        total_pixels: d.total_pixels,
        transparent_pixels: d.transparent_pixels,
        transparent_percent: d.transparent_percent,
        has_alpha: d.has_alpha,
        is_blank: d.is_blank,
        verdict: d.verdict.to_string(),
        reason: d.reason,
        confidence: d.confidence,
        dominant_hex: d.dominant_hex,
        dominant_hex_rgba: d.dominant_hex_rgba,
        dominant_rgb: d.dominant_rgb,
        color_name: d.color_name.to_string(),
        r: d.r,
        g: d.g,
        b: d.b,
        a: d.a,
        coverage_percent: d.coverage_percent,
        unique_colors: d.unique_colors,
        unique_colors_capped: d.unique_colors_capped,
        mean_hex: d.mean_hex,
        luma_min: d.luma_min,
        luma_max: d.luma_max,
        luma_mean: d.luma_mean,
        luma_stddev: d.luma_stddev,
        channel_range: d.channel_range,
        entropy: d.entropy,
        warnings: d.warnings,
        note,
        filename,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizza_ai_image_blank_detector_core::ALL_WHITE;

    fn sample(verdict: &'static str) -> Detection {
        Detection {
            width: 800,
            height: 600,
            total_pixels: 480_000,
            transparent_pixels: 0,
            transparent_percent: 0.0,
            has_alpha: false,
            is_blank: verdict != NOT_BLANK,
            verdict,
            reason: "reason".into(),
            confidence: 0.98,
            dominant_hex: "#ffffff".into(),
            dominant_hex_rgba: "#ffffffff".into(),
            dominant_rgb: "rgb(255, 255, 255)".into(),
            color_name: "white",
            r: 255,
            g: 255,
            b: 255,
            a: 255,
            coverage_percent: if verdict == NOT_BLANK { 31.5 } else { 100.0 },
            unique_colors: 1,
            unique_colors_capped: false,
            mean_hex: "#ffffff".into(),
            luma_min: 255,
            luma_max: 255,
            luma_mean: 255.0,
            luma_stddev: 0.0,
            channel_range: 0,
            entropy: 0.0,
            warnings: vec![],
        }
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_tolerance(), 2.0);
        assert_eq!(d_blank_threshold(), 99.5);
        assert!(d_ignore_transparency());
    }

    #[test]
    fn args_parse_from_a_bare_url_using_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://example.com/render.png"}"#).unwrap();
        assert_eq!(a.tolerance, 2.0);
        assert_eq!(a.blank_threshold, 99.5);
        assert!(a.ignore_transparency);
    }

    #[test]
    fn note_leads_with_the_verdict() {
        let blank = response(sample(ALL_WHITE), None);
        assert!(blank.note.starts_with("BLANK (all white)"), "{}", blank.note);
        assert!(blank.note.contains("800x600"), "{}", blank.note);
        assert!(blank.note.contains("100.00%"), "{}", blank.note);

        let busy = response(sample(NOT_BLANK), None);
        assert!(busy.note.starts_with("NOT BLANK"), "{}", busy.note);
        assert!(busy.note.contains("31.50%"), "{}", busy.note);
    }

    #[test]
    fn transparent_and_near_blank_notes_say_why() {
        assert!(response(sample(TRANSPARENT), None)
            .note
            .contains("fully transparent"));
        assert!(response(sample(NEAR_BLANK), None)
            .note
            .contains("empty apart from a small mark"));
    }

    #[test]
    fn capped_color_count_is_shown_as_a_lower_bound() {
        let mut d = sample(NOT_BLANK);
        d.unique_colors = 65_536;
        d.unique_colors_capped = true;
        assert!(
            response(d, None).note.contains("65536+ distinct"),
            "capped counts must read as a lower bound"
        );
    }

    #[test]
    fn filename_is_passed_through_when_known() {
        let resp = response(sample(ALL_WHITE), Some("render-042.png".into()));
        assert_eq!(resp.filename.as_deref(), Some("render-042.png"));
        assert!(response(sample(ALL_WHITE), None).filename.is_none());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "tolerance": { "type": "number", "minimum": 0, "maximum": 100, "default": 2.0, "description": "How far a pixel may differ per channel and still count as the same colour, as a percent of the 0-255 range, 0-100. 0 demands exactly identical pixels; the default 2 (about 5 levels) absorbs JPEG/WebP compression noise on an otherwise flat fill (default 2)." },
                    "blank_threshold": { "type": "number", "minimum": 50, "maximum": 100, "default": 99.5, "description": "Percent of pixels that must match the dominant colour before the image is called blank, 50-100. 100 flags only perfectly uniform images; the default 99.5 also flags a page that is empty apart from a small watermark (default 99.5)." },
                    "ignore_transparency": { "type": "boolean", "default": true, "description": "When true, near-transparent pixels are treated as one 'empty' colour so the junk RGB some encoders leave underneath alpha=0 cannot fake content; set false to compare the raw RGBA values as stored (default true)." }
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
