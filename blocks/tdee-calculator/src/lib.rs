//! gizza-ai/tdee-calculator — chat skill block on the shared tool abstraction.
//!
//! Estimates Basal Metabolic Rate (BMR) and Total Daily Energy Expenditure
//! (TDEE) from age, sex, weight, height and activity level. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill` and returns the pretty result JSON from
//! `core::compute_json`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_tdee_calculator_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    age: Option<f64>,
    #[serde(default)]
    sex: Option<String>,
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    activity: Option<String>,
    #[serde(default)]
    formula: Option<String>,
    #[serde(default)]
    body_fat: Option<f64>,
    #[serde(default)]
    energy_unit: Option<String>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            age: self.age,
            sex: self.sex,
            weight: self.weight,
            height: self.height,
            units: self.units,
            activity: self.activity,
            formula: self.formula,
            body_fat: self.body_fat,
            energy_unit: self.energy_unit,
        }
    }
}

const SEX: [&str; 2] = ["male", "female"];
const UNITS: [&str; 2] = ["metric", "imperial"];
const ACTIVITY: [&str; 5] = ["sedentary", "light", "moderate", "very_active", "extra_active"];
const FORMULA: [&str; 3] = ["mifflin_st_jeor", "harris_benedict", "katch_mcardle"];
const ENERGY_UNIT: [&str; 2] = ["calories", "kilojoules"];

/// Single source for the chat schema (and CLI). Every field is optional and
/// falls back to the documented default, so the tool always returns a result.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("age")
                .default(30.0)
                .min(1.0)
                .max(120.0)
                .describe("Age in whole years, e.g. 30. Default 30 (range 1–120)."),
        )
        .param(
            Param::enumv("sex", SEX)
                .default("male")
                .describe(
                    "Biological sex used by the Mifflin-St Jeor and Harris-Benedict \
                     formulas: male or female. Default male. Ignored by katch_mcardle.",
                ),
        )
        .param(
            Param::number("weight")
                .default(70.0)
                .min(0.0)
                .describe(
                    "Body weight, in kg when units=metric or lb when units=imperial. \
                     Default 70.",
                ),
        )
        .param(
            Param::number("height")
                .default(175.0)
                .min(0.0)
                .describe(
                    "Height, in cm when units=metric or total inches when units=imperial \
                     (e.g. 5'10\" = 70). Default 175.",
                ),
        )
        .param(
            Param::enumv("units", UNITS)
                .default("metric")
                .describe(
                    "Unit system for weight and height: metric (kg/cm) or imperial \
                     (lb/inches). Default metric.",
                ),
        )
        .param(
            Param::enumv("activity", ACTIVITY)
                .default("moderate")
                .describe(
                    "Activity level for the TDEE multiplier: sedentary (×1.2, little/no \
                     exercise), light (×1.375, 1–3 days/wk), moderate (×1.55, 3–5 days/wk), \
                     very_active (×1.725, 6–7 days/wk), extra_active (×1.9, hard daily \
                     training or physical job). Default moderate.",
                ),
        )
        .param(
            Param::enumv("formula", FORMULA)
                .default("mifflin_st_jeor")
                .describe(
                    "BMR equation: mifflin_st_jeor (default, most accurate for modern \
                     populations), harris_benedict (revised 1984), or katch_mcardle \
                     (uses lean body mass from body_fat, ignores age/sex/height).",
                ),
        )
        .param(
            Param::number("body_fat")
                .default(20.0)
                .min(0.0)
                .max(100.0)
                .describe(
                    "Body-fat percentage (0–100), only used by the katch_mcardle formula. \
                     Default 20.",
                ),
        )
        .param(
            Param::enumv("energy_unit", ENERGY_UNIT)
                .default("calories")
                .describe(
                    "Unit for all returned energy figures: calories (kcal) or kilojoules \
                     (1 kcal = 4.184 kJ). Default calories.",
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
    name = "gizza-ai/tdee-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate BMR and total daily energy expenditure from age, sex, weight, height and activity.",
    skill(
        description = "Estimate Basal Metabolic Rate (BMR) and Total Daily Energy Expenditure (TDEE). Pass age, sex (male/female), weight and height (units=metric for kg/cm or imperial for lb/inches), activity (sedentary/light/moderate/very_active/extra_active), and optionally formula (mifflin_st_jeor default, harris_benedict, or katch_mcardle with body_fat) and energy_unit (calories or kilojoules). Every parameter is optional with a sensible default. Returns bmr, tdee, the activity multiplier, bmi + bmi_category, calorie goals for cutting/maintaining/bulking (goals), tdee at all five activity levels (tdee_by_activity), and a plain-language summary. Estimates for planning, not medical advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "tdee-calculator", |a: Args| {
            gizza_ai_tdee_calculator_core::compute_json(&a.inputs()).map_err(SkillError::InvalidArgs)
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
                    "age": { "type": "number", "minimum": 1, "maximum": 120, "default": 30.0, "description": "Age in whole years, e.g. 30. Default 30 (range 1–120)." },
                    "sex": { "type": "string", "enum": ["male","female"], "default": "male", "description": "Biological sex used by the Mifflin-St Jeor and Harris-Benedict formulas: male or female. Default male. Ignored by katch_mcardle." },
                    "weight": { "type": "number", "minimum": 0, "default": 70.0, "description": "Body weight, in kg when units=metric or lb when units=imperial. Default 70." },
                    "height": { "type": "number", "minimum": 0, "default": 175.0, "description": "Height, in cm when units=metric or total inches when units=imperial (e.g. 5'10\" = 70). Default 175." },
                    "units": { "type": "string", "enum": ["metric","imperial"], "default": "metric", "description": "Unit system for weight and height: metric (kg/cm) or imperial (lb/inches). Default metric." },
                    "activity": { "type": "string", "enum": ["sedentary","light","moderate","very_active","extra_active"], "default": "moderate", "description": "Activity level for the TDEE multiplier: sedentary (×1.2, little/no exercise), light (×1.375, 1–3 days/wk), moderate (×1.55, 3–5 days/wk), very_active (×1.725, 6–7 days/wk), extra_active (×1.9, hard daily training or physical job). Default moderate." },
                    "formula": { "type": "string", "enum": ["mifflin_st_jeor","harris_benedict","katch_mcardle"], "default": "mifflin_st_jeor", "description": "BMR equation: mifflin_st_jeor (default, most accurate for modern populations), harris_benedict (revised 1984), or katch_mcardle (uses lean body mass from body_fat, ignores age/sex/height)." },
                    "body_fat": { "type": "number", "minimum": 0, "maximum": 100, "default": 20.0, "description": "Body-fat percentage (0–100), only used by the katch_mcardle formula. Default 20." },
                    "energy_unit": { "type": "string", "enum": ["calories","kilojoules"], "default": "calories", "description": "Unit for all returned energy figures: calories (kcal) or kilojoules (1 kcal = 4.184 kJ). Default calories." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
