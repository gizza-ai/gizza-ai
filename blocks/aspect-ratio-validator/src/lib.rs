//! gizza-ai/aspect-ratio-validator — compute a width×height aspect ratio and
//! check it against a target within a tolerance.
//!
//! Pure arithmetic: the descriptor single-sources the chat schema and the CLI,
//! and the same core powers the standalone page. No host calls, so it runs on
//! ALL backends including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_aspect_ratio_validator_core::{
    analyze, Options, DEFAULT_TOLERANCE_PERCENT, MAX_DIMENSION, MAX_TOLERANCE_PERCENT,
};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
struct Args {
    width: f64,
    height: f64,
    #[serde(default)]
    target: String,
    #[serde(default = "d_tolerance")]
    tolerance_percent: f64,
    #[serde(default)]
    orientation_agnostic: bool,
    #[serde(default)]
    even_dimensions: bool,
}

fn d_tolerance() -> f64 {
    DEFAULT_TOLERANCE_PERCENT
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            width: a.width,
            height: a.height,
            target: a.target,
            tolerance_percent: a.tolerance_percent,
            orientation_agnostic: a.orientation_agnostic,
            even_dimensions: a.even_dimensions,
        }
    }
}

/// Single source for the chat schema (and the CLI + page query params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("width")
                .required()
                .min(1.0)
                .max(MAX_DIMENSION)
                .describe("Required. The asset's width in pixels (any unit works — a ratio is unit-free), e.g. 1920."),
        )
        .param(
            Param::number("height")
                .required()
                .min(1.0)
                .max(MAX_DIMENSION)
                .describe("Required. The asset's height in the same unit as width, e.g. 1080."),
        )
        .param(
            Param::string("target")
                .describe("The aspect ratio the asset is supposed to be. Accepts 16:9, 4/5, 1.85:1, 1920x1080 or a bare decimal like 1.7778. Leave it out to just report the ratio, its nearest standard and the orientation with no PASS/FAIL verdict."),
        )
        .param(
            Param::number("tolerance_percent")
                .min(0.0)
                .max(MAX_TOLERANCE_PERCENT)
                .default(DEFAULT_TOLERANCE_PERCENT)
                .describe("Allowed deviation from the target, as a percentage of the target ratio (default 1). 0 demands an exact ratio; 1 absorbs the usual off-by-one-pixel rounding (1920x1081 is 0.09% off 16:9). 0-100."),
        )
        .param(
            Param::boolean("orientation_agnostic")
                .default(false)
                .describe("When true, a rotated asset also passes — 1080x1920 satisfies a 16:9 target because 9:16 is the same ratio turned on its side. Default false (orientation must match)."),
        )
        .param(
            Param::boolean("even_dimensions")
                .default(false)
                .describe("When true, round the suggested crop/pad dimensions to even numbers, which most video encoders (H.264/H.265) require. Default false (whole pixels)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/aspect-ratio-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check a width and height against a target aspect ratio and report PASS or FAIL.",
    skill(
        description = "Compute an image, video or screen's aspect ratio from its width and height and check it against a target ratio within a tolerance. Parameters: width and height (required, 1-1000000, any consistent unit); target is the required ratio written as 16:9, 4/5, 1.85:1, 1920x1080 or a bare decimal like 1.7778 (omit it for a report-only run with no verdict); tolerance_percent (default 1) is the allowed deviation as a percentage of the target; orientation_agnostic=true lets a rotated asset pass (1080x1920 satisfies a 16:9 target); even_dimensions=true rounds the crop/pad suggestions to even numbers for video encoders. Returns status (PASS/FAIL, or INFO with no target), pass, the GCD-reduced ratio (e.g. 16:9), ratio_decimal and ratio_x_to_1, orientation (landscape/portrait/square), the nearest standard ratio with its name and deviation, target_ratio and target_decimal, signed deviation_percent (positive means too wide), reason (ok/too_wide/too_tall), orientation_flipped, the crop_width/crop_height that fit the target inside the current frame with the crop_loss_percent that discards, the pad_width/pad_height that contain it, and a one-line summary. Pure arithmetic — nothing is fetched or uploaded. To validate an image file, read its dimensions with the image-info tool first.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } — the report is
        // a struct, so the LLM reads the fields straight out of it.
        match run_skill(&body, "aspect-ratio-validator", |a: Args| {
            analyze(&Options::from(a)).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn defaults_match_the_descriptor() {
        let a = args(r#"{"width":1920,"height":1080}"#);
        assert_eq!(a.tolerance_percent, 1.0);
        assert_eq!(a.target, "");
        assert!(!a.orientation_agnostic);
        assert!(!a.even_dimensions);
    }

    #[test]
    fn args_flow_into_the_core_options() {
        let a = args(
            r#"{"width":1080,"height":1920,"target":"16:9","tolerance_percent":0,"orientation_agnostic":true,"even_dimensions":true}"#,
        );
        let o = Options::from(a);
        assert_eq!(o.width, 1080.0);
        assert_eq!(o.target, "16:9");
        assert_eq!(o.tolerance_percent, 0.0);
        assert!(o.orientation_agnostic);
        assert!(o.even_dimensions);
        assert_eq!(analyze(&o).unwrap().status, "PASS");
    }

    #[test]
    fn width_and_height_are_mandatory() {
        let e = serde_json::from_str::<Args>(r#"{"height":1080}"#).unwrap_err();
        assert!(e.to_string().contains("width"), "{e}");
        let e = serde_json::from_str::<Args>(r#"{"width":1920}"#).unwrap_err();
        assert!(e.to_string().contains("height"), "{e}");
    }

    #[test]
    fn a_bad_target_fails_before_a_verdict_is_reported() {
        let o = Options::from(args(r#"{"width":1920,"height":1080,"target":"square-ish"}"#));
        assert!(analyze(&o).unwrap_err().contains("use a form like 16:9"));
    }

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "width": { "type": "number", "minimum": 1, "maximum": 1000000, "description": "Required. The asset's width in pixels (any unit works — a ratio is unit-free), e.g. 1920." },
                    "height": { "type": "number", "minimum": 1, "maximum": 1000000, "description": "Required. The asset's height in the same unit as width, e.g. 1080." },
                    "target": { "type": "string", "description": "The aspect ratio the asset is supposed to be. Accepts 16:9, 4/5, 1.85:1, 1920x1080 or a bare decimal like 1.7778. Leave it out to just report the ratio, its nearest standard and the orientation with no PASS/FAIL verdict." },
                    "tolerance_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 1.0, "description": "Allowed deviation from the target, as a percentage of the target ratio (default 1). 0 demands an exact ratio; 1 absorbs the usual off-by-one-pixel rounding (1920x1081 is 0.09% off 16:9). 0-100." },
                    "orientation_agnostic": { "type": "boolean", "default": false, "description": "When true, a rotated asset also passes — 1080x1920 satisfies a 16:9 target because 9:16 is the same ratio turned on its side. Default false (orientation must match)." },
                    "even_dimensions": { "type": "boolean", "default": false, "description": "When true, round the suggested crop/pad dimensions to even numbers, which most video encoders (H.264/H.265) require. Default false (whole pixels)." }
                },
                "required": ["width", "height"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
