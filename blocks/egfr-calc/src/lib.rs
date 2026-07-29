//! gizza-ai/egfr-calc — chat skill block on the shared tool abstraction.
//!
//! Estimates glomerular filtration rate (eGFR) from serum creatinine, age and
//! sex using the race-free CKD-EPI creatinine equations (2021 default, 2009 for
//! comparison). The chat schema is single-sourced from `descriptor()` (which
//! also drives the CLI); `handle()` delegates to `block_utils::run_skill` and
//! returns the pretty result JSON from `core::compute_json`. Pure compute — no
//! host calls. Informational estimate, not a diagnosis or medical advice.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_egfr_calc_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    creatinine: Option<f64>,
    #[serde(default)]
    creatinine_unit: Option<String>,
    #[serde(default)]
    age: Option<f64>,
    #[serde(default)]
    sex: Option<String>,
    #[serde(default)]
    equation: Option<String>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            creatinine: self.creatinine,
            creatinine_unit: self.creatinine_unit,
            age: self.age,
            sex: self.sex,
            equation: self.equation,
        }
    }
}

const CREATININE_UNIT: [&str; 2] = ["mg/dL", "µmol/L"];
const SEX: [&str; 2] = ["male", "female"];
const EQUATION: [&str; 2] = ["ckd_epi_2021", "ckd_epi_2009"];

/// Single source for the chat schema (and CLI). Every field is optional and
/// falls back to the documented default, so the tool always returns a result.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("creatinine")
                .default(1.0)
                .min(0.0)
                .describe(
                    "Serum creatinine value, in the unit given by creatinine_unit \
                     (mg/dL by default). Must be an IDMS-standardized lab value. \
                     Default 1.0.",
                ),
        )
        .param(
            Param::enumv("creatinine_unit", CREATININE_UNIT)
                .default("mg/dL")
                .describe(
                    "Unit of the creatinine value: mg/dL (US) or µmol/L (SI, \
                     converted by ÷88.42). Default mg/dL.",
                ),
        )
        .param(
            Param::number("age")
                .default(50.0)
                .min(18.0)
                .max(120.0)
                .describe(
                    "Age in whole years. CKD-EPI is validated for adults, so ages \
                     under 18 are rejected. Default 50 (range 18–120).",
                ),
        )
        .param(
            Param::enumv("sex", SEX)
                .default("male")
                .describe("Biological sex used by the equation: male or female. Default male."),
        )
        .param(
            Param::enumv("equation", EQUATION)
                .default("ckd_epi_2021")
                .describe(
                    "Which race-free CKD-EPI creatinine equation to use: \
                     ckd_epi_2021 (default, the current NKF/ASN-recommended US \
                     standard) or ckd_epi_2009 (older, for historical comparison; \
                     the 2009 race coefficient is deliberately omitted).",
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
    name = "gizza-ai/egfr-calc",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate kidney filtration rate (eGFR) from creatinine, age and sex using race-free CKD-EPI.",
    skill(
        description = "Estimate glomerular filtration rate (eGFR) from serum creatinine, age and sex using the race-free CKD-EPI creatinine equations. Pass creatinine (with creatinine_unit=mg/dL or µmol/L), age (adults 18–120), sex (male/female), and optionally equation (ckd_epi_2021 default, the current US standard, or ckd_epi_2009 for comparison — its race coefficient is deliberately omitted). Every parameter is optional with a sensible default. Returns egfr in mL/min/1.73 m² (whole number), the KDIGO GFR category (gfr_stage G1–G5) with a plain-language stage_description, the creatinine used in mg/dL, and a summary. Informational estimate, not a diagnosis or medical advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "egfr-calc", |a: Args| {
            gizza_ai_egfr_calc_core::compute_json(&a.inputs()).map_err(SkillError::InvalidArgs)
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
                    "creatinine": { "type": "number", "minimum": 0, "default": 1.0, "description": "Serum creatinine value, in the unit given by creatinine_unit (mg/dL by default). Must be an IDMS-standardized lab value. Default 1.0." },
                    "creatinine_unit": { "type": "string", "enum": ["mg/dL","µmol/L"], "default": "mg/dL", "description": "Unit of the creatinine value: mg/dL (US) or µmol/L (SI, converted by ÷88.42). Default mg/dL." },
                    "age": { "type": "number", "minimum": 18, "maximum": 120, "default": 50.0, "description": "Age in whole years. CKD-EPI is validated for adults, so ages under 18 are rejected. Default 50 (range 18–120)." },
                    "sex": { "type": "string", "enum": ["male","female"], "default": "male", "description": "Biological sex used by the equation: male or female. Default male." },
                    "equation": { "type": "string", "enum": ["ckd_epi_2021","ckd_epi_2009"], "default": "ckd_epi_2021", "description": "Which race-free CKD-EPI creatinine equation to use: ckd_epi_2021 (default, the current NKF/ASN-recommended US standard) or ckd_epi_2009 (older, for historical comparison; the 2009 race coefficient is deliberately omitted)." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
