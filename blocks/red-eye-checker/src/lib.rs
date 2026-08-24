//! gizza-ai/red-eye-checker — locate flash red-eye in a portrait.
//!
//! URL/ref image analyser: the descriptor single-sources the chat schema and the
//! CLI; the handler resolves an image, runs the pure Rust core, and returns the
//! report as JSON. Detector only — it never rewrites pixels.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_red_eye_checker_core::{analyze, Options, Region, Report, Sensitivity};
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
    #[serde(default = "d_sensitivity")]
    sensitivity: String,
    #[serde(default = "d_min_radius")]
    min_radius: u32,
    #[serde(default = "d_max_radius")]
    max_radius: u32,
    #[serde(default = "d_max_regions")]
    max_regions: u32,
}

fn d_sensitivity() -> String {
    "medium".into()
}
fn d_min_radius() -> u32 {
    Options::default().min_radius
}
fn d_max_radius() -> u32 {
    Options::default().max_radius
}
fn d_max_regions() -> u32 {
    Options::default().max_regions
}

#[derive(Serialize)]
struct Resp {
    width: u32,
    height: u32,
    candidate_count: usize,
    sensitivity: &'static str,
    regions: Vec<Region>,
    warnings: Vec<String>,
    note: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("sensitivity", ["low", "medium", "high"])
                .default("medium")
                .describe("How eagerly a pixel counts as flash red: low (only bright, strongly saturated red — fewest false positives), medium (default, typical phone/compact-camera red-eye), high (also flags dim, partly-corrected or amber eye, at the cost of more false positives)."),
        )
        .param(
            Param::integer("min_radius")
                .min(1.0)
                .max(80.0)
                .default(3)
                .describe("Smallest red-eye radius to report, in pixels, 1-80; raise it to ignore red speckle and JPEG noise in a large photo (default 3)."),
        )
        .param(
            Param::integer("max_radius")
                .min(1.0)
                .max(300.0)
                .default(80)
                .describe("Largest red-eye radius to report, in pixels, 1-300; must be at least min_radius, and lowering it rejects big red objects like clothing (default 80)."),
        )
        .param(
            Param::integer("max_regions")
                .min(1.0)
                .max(100.0)
                .default(20)
                .describe("Maximum number of regions to list, 1-100, highest confidence first; candidate_count still reports how many were found before this cap (default 20)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/red-eye-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect flash red-eye regions in a photo and report where they are.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Check a portrait or flash photo for red-eye and report every candidate region. Provide the image as url or ref (PNG or JPEG). Red-dominant, saturated, bright pixels are grouped into connected regions and filtered by pupil-like size and shape. Returns width, height, candidate_count, the sensitivity used, a regions list (center_x, center_y, radius_px, area_px, average_red, confidence 0-1, highest confidence first) and warnings explaining anything skipped. Parameters: sensitivity=low|medium|high (default medium), min_radius 1-80 px (default 3), max_radius 1-300 px (default 80), max_regions 1-100 (default 20). This is a detector only: it reports where red-eye is, it does not remove or edit it.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("red-eye-checker")?;
    let opts = options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let report = analyze(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&response(report))
        .map_err(|e| SkillError::Serialize(format!("serialize red-eye-checker response: {e}")))
}

/// Args → core options. Sensitivity is parsed here so a bad enum value fails
/// with the core's message before anything is fetched.
fn options(args: &Args) -> Result<Options, String> {
    let opts = Options {
        sensitivity: Sensitivity::parse(&args.sensitivity)?,
        min_radius: args.min_radius,
        max_radius: args.max_radius,
        max_regions: args.max_regions,
    };
    opts.validate()?;
    Ok(opts)
}

fn note(r: &Report) -> String {
    if r.candidate_count == 0 {
        return format!(
            "No red-eye found in this {}x{} image at sensitivity '{}'.",
            r.width, r.height, r.sensitivity
        );
    }
    let best = &r.regions[0];
    let plural = if r.candidate_count == 1 {
        "region"
    } else {
        "regions"
    };
    format!(
        "Found {} red-eye {plural} in this {}x{} image at sensitivity '{}'. The strongest is at \
         ({}, {}) with a {:.1} px radius and confidence {:.3}.",
        r.candidate_count,
        r.width,
        r.height,
        r.sensitivity,
        best.center_x,
        best.center_y,
        best.radius_px,
        best.confidence
    )
}

fn response(r: Report) -> Resp {
    let note = note(&r);
    Resp {
        width: r.width,
        height: r.height,
        candidate_count: r.candidate_count,
        sensitivity: r.sensitivity,
        regions: r.regions,
        warnings: r.warnings,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(x: u32, y: u32, conf: f64) -> Region {
        Region {
            center_x: x,
            center_y: y,
            radius_px: 6.5,
            area_px: 133,
            average_red: 223.4,
            confidence: conf,
        }
    }

    fn report(regions: Vec<Region>) -> Report {
        Report {
            width: 640,
            height: 480,
            candidate_count: regions.len(),
            sensitivity: "medium",
            regions,
            warnings: vec![],
        }
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_sensitivity(), "medium");
        assert_eq!(d_min_radius(), 3);
        assert_eq!(d_max_radius(), 80);
        assert_eq!(d_max_regions(), 20);
    }

    #[test]
    fn args_parse_from_a_bare_url_using_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://example.com/flash.jpg"}"#).unwrap();
        assert_eq!(a.sensitivity, "medium");
        assert_eq!(a.min_radius, 3);
        assert_eq!(a.max_radius, 80);
        assert_eq!(a.max_regions, 20);
        assert_eq!(options(&a).unwrap(), Options::default());
    }

    #[test]
    fn bad_params_are_rejected_before_the_image_is_fetched() {
        let a: Args =
            serde_json::from_str(r#"{"url":"https://example.com/a.png","sensitivity":"ultra"}"#)
                .unwrap();
        assert!(options(&a).unwrap_err().contains("sensitivity"));
        let a: Args =
            serde_json::from_str(r#"{"url":"https://example.com/a.png","min_radius":90}"#).unwrap();
        assert!(options(&a).unwrap_err().contains("min_radius"));
        let a: Args = serde_json::from_str(
            r#"{"url":"https://example.com/a.png","min_radius":40,"max_radius":10}"#,
        )
        .unwrap();
        assert!(options(&a).unwrap_err().contains("must not exceed"));
    }

    #[test]
    fn note_summarizes_hits_and_misses() {
        let hit = response(report(vec![region(210, 180, 0.812), region(330, 182, 0.74)]));
        assert!(hit.note.contains("Found 2 red-eye regions"), "{}", hit.note);
        assert!(hit.note.contains("(210, 180)"), "{}", hit.note);
        assert!(hit.note.contains("confidence 0.812"), "{}", hit.note);
        assert_eq!(hit.candidate_count, 2);

        let one = response(report(vec![region(210, 180, 0.9)]));
        assert!(one.note.contains("1 red-eye region"), "{}", one.note);

        let miss = response(report(vec![]));
        assert!(miss.note.contains("No red-eye found"), "{}", miss.note);
        assert!(miss.note.contains("640x480"), "{}", miss.note);
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "sensitivity": { "type": "string", "enum": ["low", "medium", "high"], "default": "medium", "description": "How eagerly a pixel counts as flash red: low (only bright, strongly saturated red — fewest false positives), medium (default, typical phone/compact-camera red-eye), high (also flags dim, partly-corrected or amber eye, at the cost of more false positives)." },
                    "min_radius": { "type": "integer", "minimum": 1, "maximum": 80, "default": 3, "description": "Smallest red-eye radius to report, in pixels, 1-80; raise it to ignore red speckle and JPEG noise in a large photo (default 3)." },
                    "max_radius": { "type": "integer", "minimum": 1, "maximum": 300, "default": 80, "description": "Largest red-eye radius to report, in pixels, 1-300; must be at least min_radius, and lowering it rejects big red objects like clothing (default 80)." },
                    "max_regions": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20, "description": "Maximum number of regions to list, 1-100, highest confidence first; candidate_count still reports how many were found before this cap (default 20)." }
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
