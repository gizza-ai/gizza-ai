//! gizza-ai/ideal-weight — chat skill block on the shared tool abstraction.
//!
//! Estimates adult ideal-body-weight ranges from height and sex using the four
//! classic clinical equations (Hamwi, Devine, Robinson, Miller) plus a
//! healthy-BMI weight band, with an optional body-frame adjustment. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` delegates to `block_utils::run_skill` and returns the pretty
//! result JSON from `core::compute_json`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ideal_weight_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    sex: Option<String>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    frame: Option<String>,
    #[serde(default)]
    wrist: Option<f64>,
    #[serde(default)]
    age: Option<f64>,
    #[serde(default)]
    bmi_min: Option<f64>,
    #[serde(default)]
    bmi_max: Option<f64>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            height: self.height,
            sex: self.sex,
            units: self.units,
            frame: self.frame,
            wrist: self.wrist,
            age: self.age,
            bmi_min: self.bmi_min,
            bmi_max: self.bmi_max,
        }
    }
}

const SEX: [&str; 2] = ["male", "female"];
const UNITS: [&str; 2] = ["metric", "imperial"];
const FRAME: [&str; 4] = ["small", "medium", "large", "auto"];

/// Single source for the chat schema (and CLI). Every field is optional and
/// falls back to the documented default, so the tool always returns a result.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("height")
                .default(175.0)
                .min(0.0)
                .describe(
                    "Height, in cm when units=metric or total inches when units=imperial \
                     (e.g. 5'10\" = 70). Default 175 cm (69 in when imperial). Accepted \
                     range 122–250 cm / 48–98.4 in — the equations are adult-only.",
                ),
        )
        .param(
            Param::enumv("sex", SEX)
                .default("male")
                .describe(
                    "Biological sex the equations were published for: male or female. \
                     Default male.",
                ),
        )
        .param(
            Param::enumv("units", UNITS)
                .default("metric")
                .describe(
                    "Unit system for the height and wrist inputs: metric (cm) or imperial \
                     (inches). Results are always reported in both kg and lb. Default metric.",
                ),
        )
        .param(
            Param::enumv("frame", FRAME)
                .default("medium")
                .describe(
                    "Body frame size, applied as a weight adjustment to every formula: \
                     small (−10%), medium (no adjustment, default), large (+10%), or auto \
                     to derive the frame from the wrist measurement.",
                ),
        )
        .param(
            Param::number("wrist")
                .min(0.0)
                .describe(
                    "Wrist circumference measured just below the wrist bone, in cm when \
                     units=metric or inches when units=imperial (e.g. 17 cm / 6.7 in). \
                     Required when frame=auto, ignored otherwise. Optional.",
                ),
        )
        .param(
            Param::number("age")
                .min(0.0)
                .max(120.0)
                .describe(
                    "Age in years, e.g. 35. Optional and not used by any formula — it only \
                     adds a note when under 18, since these are adult equations.",
                ),
        )
        .param(
            Param::number("bmi_min")
                .default(18.5)
                .min(0.0)
                .describe(
                    "Lower BMI bound for the healthy-weight range. Default 18.5 (WHO). Use \
                     e.g. 18.5 with bmi_max=23 for the WHO Asian cutoffs.",
                ),
        )
        .param(
            Param::number("bmi_max")
                .default(24.9)
                .min(0.0)
                .describe(
                    "Upper BMI bound for the healthy-weight range. Default 24.9 (WHO). Must \
                     be greater than bmi_min.",
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
    name = "gizza-ai/ideal-weight",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate ideal body-weight ranges from height and sex using the Devine, Robinson, Miller and Hamwi formulas plus a healthy BMI range.",
    skill(
        description = "Estimate adult ideal body weight (IBW) from height and sex. Pass height (units=metric for cm or imperial for total inches), sex (male/female), and optionally frame (small/medium/large for a ∓10% body-frame adjustment, or auto to derive it from a wrist measurement), wrist, age, and the healthy BMI bounds bmi_min/bmi_max. Every parameter is optional with a sensible default. Returns all four classic formulas side by side (Hamwi 1964, Devine 1974, Robinson 1983, Miller 1983) in kg and lb with the BMI each represents, their average and min-max spread, the healthy-BMI weight range, per-input caveats, and a plain-language summary. Adult estimates for planning, not medical advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ideal-weight", |a: Args| {
            gizza_ai_ideal_weight_core::compute_json(&a.inputs()).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "height": { "type": "number", "minimum": 0, "default": 175.0, "description": "Height, in cm when units=metric or total inches when units=imperial (e.g. 5'10\" = 70). Default 175 cm (69 in when imperial). Accepted range 122–250 cm / 48–98.4 in — the equations are adult-only." },
                    "sex": { "type": "string", "enum": ["male","female"], "default": "male", "description": "Biological sex the equations were published for: male or female. Default male." },
                    "units": { "type": "string", "enum": ["metric","imperial"], "default": "metric", "description": "Unit system for the height and wrist inputs: metric (cm) or imperial (inches). Results are always reported in both kg and lb. Default metric." },
                    "frame": { "type": "string", "enum": ["small","medium","large","auto"], "default": "medium", "description": "Body frame size, applied as a weight adjustment to every formula: small (−10%), medium (no adjustment, default), large (+10%), or auto to derive the frame from the wrist measurement." },
                    "wrist": { "type": "number", "minimum": 0, "description": "Wrist circumference measured just below the wrist bone, in cm when units=metric or inches when units=imperial (e.g. 17 cm / 6.7 in). Required when frame=auto, ignored otherwise. Optional." },
                    "age": { "type": "number", "minimum": 0, "maximum": 120, "description": "Age in years, e.g. 35. Optional and not used by any formula — it only adds a note when under 18, since these are adult equations." },
                    "bmi_min": { "type": "number", "minimum": 0, "default": 18.5, "description": "Lower BMI bound for the healthy-weight range. Default 18.5 (WHO). Use e.g. 18.5 with bmi_max=23 for the WHO Asian cutoffs." },
                    "bmi_max": { "type": "number", "minimum": 0, "default": 24.9, "description": "Upper BMI bound for the healthy-weight range. Default 24.9 (WHO). Must be greater than bmi_min." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
