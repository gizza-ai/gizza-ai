//! gizza-ai/aspect-ratio-calc — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_aspect_ratio_calc_core::run;
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_ratio")]
    ratio: String,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
    #[serde(default)]
    rounding: String,
    #[serde(default)]
    output_format: String,
}
fn default_ratio() -> String {
    "16:9".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("ratio").default("16:9").describe("Target aspect ratio, written as '16:9', '4/5', '1.85:1', '1920x1080' or a bare decimal like '1.7778'. Paired with a width or a height it solves the missing dimension; on its own it is normalised and described. Leave it empty and pass BOTH width and height to reduce that resolution to its ratio instead."))
        .param(Param::number("width").min(0.0).max(1000000.0).describe("Known width in pixels, e.g. 1920. Leave it out (or pass 0) to solve for it from the ratio and the height. Passing it together with height AND a ratio shows both ways to reach that ratio. Max 1000000."))
        .param(Param::number("height").min(0.0).max(1000000.0).describe("Known height in pixels, e.g. 1080. Leave it out (or pass 0) to solve for it from the ratio and the width. Passing it together with width AND a ratio shows both ways to reach that ratio. Max 1000000."))
        .param(Param::enumv("rounding", ["nearest", "up", "down", "even", "exact"]).default("nearest").describe("How the solved dimension is snapped to a usable number. 'nearest' (default) rounds to the closest whole pixel; 'up' never crops; 'down' never overflows a size budget; 'even' snaps to an even number, which H.264/H.265 encoders require; 'exact' keeps the fraction and reports 4 decimals. Ignored when no dimension has to be solved."))
        .param(Param::enumv("output_format", ["summary", "dimensions", "ratio", "decimal", "css", "json"]).default("summary").describe("'summary' (default): the solved dimensions, the reduced ratio, orientation, the nearest standard ratio with its deviation, total pixels, megapixels, diagonal and the CSS declaration. 'dimensions': just WIDTHxHEIGHT (needs a width or a height). 'ratio': just the reduced ratio, e.g. '16:9'. 'decimal': just width divided by height, to 4 decimals. 'css': an 'aspect-ratio' declaration plus the legacy padding-top ratio box. 'json': every field as a JSON object."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, SkillError> {
    run(&a.ratio, a.width, a.height, &a.rounding, &a.output_format).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct AspectRatioCalc;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/aspect-ratio-calc",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Solve a missing width or height from an aspect ratio, or reduce a resolution to its ratio.",
    skill(
        description = "Aspect ratio calculator. Give a target ratio ('16:9', '4/5', '1.85:1', '1920x1080' or a bare decimal like '1.7778') plus one dimension and it solves the other; give both width and height with no ratio and it reduces them to the smallest whole-number ratio; give a ratio alone and it normalises it. With a ratio AND both dimensions it shows both ways to reach the ratio — keep the width, or keep the height. Every answer names the nearest standard ratio (16:9, 4:3, 3:2, 1.85:1, 2.39:1, 9:16, ISO A-series paper and 19 more) with its percentage deviation, plus orientation, total pixels, megapixels, the diagonal and a ready-to-paste CSS aspect-ratio declaration. 'rounding' picks how a solved dimension is snapped — nearest (default), up, down, even (H.264/H.265 need even sides) or exact. 'output_format' selects summary (default), dimensions, ratio, decimal, css or json. Dimensions max 1000000. Pure arithmetic, no network.",
        parameters = schema_json()
    ),
)]
impl AspectRatioCalc {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "aspect-ratio-calc", run_args) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
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
                    "ratio":         { "type": "string", "default": "16:9", "description": "Target aspect ratio, written as '16:9', '4/5', '1.85:1', '1920x1080' or a bare decimal like '1.7778'. Paired with a width or a height it solves the missing dimension; on its own it is normalised and described. Leave it empty and pass BOTH width and height to reduce that resolution to its ratio instead." },
                    "width":         { "type": "number", "minimum": 0, "maximum": 1000000, "description": "Known width in pixels, e.g. 1920. Leave it out (or pass 0) to solve for it from the ratio and the height. Passing it together with height AND a ratio shows both ways to reach that ratio. Max 1000000." },
                    "height":        { "type": "number", "minimum": 0, "maximum": 1000000, "description": "Known height in pixels, e.g. 1080. Leave it out (or pass 0) to solve for it from the ratio and the width. Passing it together with width AND a ratio shows both ways to reach that ratio. Max 1000000." },
                    "rounding":      { "type": "string", "enum": ["nearest", "up", "down", "even", "exact"], "default": "nearest", "description": "How the solved dimension is snapped to a usable number. 'nearest' (default) rounds to the closest whole pixel; 'up' never crops; 'down' never overflows a size budget; 'even' snaps to an even number, which H.264/H.265 encoders require; 'exact' keeps the fraction and reports 4 decimals. Ignored when no dimension has to be solved." },
                    "output_format": { "type": "string", "enum": ["summary", "dimensions", "ratio", "decimal", "css", "json"], "default": "summary", "description": "'summary' (default): the solved dimensions, the reduced ratio, orientation, the nearest standard ratio with its deviation, total pixels, megapixels, diagonal and the CSS declaration. 'dimensions': just WIDTHxHEIGHT (needs a width or a height). 'ratio': just the reduced ratio, e.g. '16:9'. 'decimal': just width divided by height, to 4 decimals. 'css': an 'aspect-ratio' declaration plus the legacy padding-top ratio box. 'json': every field as a JSON object." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_args_solves_the_missing_height() {
        let a: Args = serde_json::from_str(
            r#"{"ratio": "16:9", "width": 1920, "output_format": "dimensions"}"#,
        )
        .unwrap();
        assert_eq!(run_args(a).unwrap(), "1920x1080");
    }

    #[test]
    fn run_args_defaults_the_ratio_to_16_9() {
        let a: Args =
            serde_json::from_str(r#"{"height": 1080, "output_format": "dimensions"}"#).unwrap();
        assert_eq!(run_args(a).unwrap(), "1920x1080");
    }

    #[test]
    fn run_args_simplifies_a_resolution_when_the_ratio_is_blank() {
        let a: Args = serde_json::from_str(
            r#"{"ratio": "", "width": 2560, "height": 1600, "output_format": "ratio"}"#,
        )
        .unwrap();
        assert_eq!(run_args(a).unwrap(), "8:5");
    }

    #[test]
    fn run_args_honours_the_rounding_mode() {
        let a: Args = serde_json::from_str(
            r#"{"ratio": "1.85:1", "width": 1920, "rounding": "down", "output_format": "dimensions"}"#,
        )
        .unwrap();
        assert_eq!(run_args(a).unwrap(), "1920x1037");
    }

    #[test]
    fn run_args_rejects_bad_enums_and_ratios() {
        let a: Args = serde_json::from_str(r#"{"ratio": "16:9", "rounding": "sideways"}"#).unwrap();
        assert!(run_args(a).is_err());
        let a: Args = serde_json::from_str(r#"{"ratio": "banana", "width": 100}"#).unwrap();
        assert!(run_args(a).is_err());
    }
}
