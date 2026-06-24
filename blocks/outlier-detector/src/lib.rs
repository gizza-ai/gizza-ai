//! gizza-ai/outlier-detector — flag outliers in a list of numbers (z-score + IQR).
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_outlier_detector_core::detect;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    numbers: String,
    #[serde(default = "default_z")]
    z_threshold: f64,
    #[serde(default = "default_k")]
    iqr_k: f64,
}

fn default_z() -> f64 {
    3.0
}
fn default_k() -> f64 {
    1.5
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("numbers")
                .required()
                .describe("The numbers to scan, separated by spaces, commas, semicolons, or newlines."),
        )
        .param(
            Param::number("z_threshold")
                .default(3.0)
                .min(0.0)
                .describe("Z-score (standard-score) cutoff: a value more than this many sample standard deviations from the mean is flagged. Default 3. Also used as the cutoff for the robust modified z-score (median/MAD) method."),
        )
        .param(
            Param::number("iqr_k")
                .default(1.5)
                .min(0.0)
                .describe("IQR multiplier for Tukey's fences: values below Q1−k·IQR or above Q3+k·IQR are flagged. Default 1.5."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct OutlierDetector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/outlier-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag outliers in a list of numbers (z-score + IQR)",
    skill(
        description = "Flag outliers in a list of numbers (separated by spaces, commas, semicolons, or newlines) using three standard methods: the z-score method (more than z_threshold sample standard deviations from the mean, default 3), the robust modified z-score method (median + MAD, same threshold), and the IQR method (Tukey's fences: below Q1−k·IQR or above Q3+k·IQR, default k=1.5). Returns the mean, standard deviation, median, MAD, quartiles and fences plus the flagged values with their indices for each method. Runs locally.",
        parameters = schema_json()
    ),
)]
impl OutlierDetector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "outlier-detector", |a: Args| {
            detect(&a.numbers, a.z_threshold, a.iqr_k).map_err(SkillError::InvalidArgs)
        }) {
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
                    "numbers": { "type": "string", "description": "The numbers to scan, separated by spaces, commas, semicolons, or newlines." },
                    "z_threshold": { "type": "number", "minimum": 0, "default": 3.0, "description": "Z-score (standard-score) cutoff: a value more than this many sample standard deviations from the mean is flagged. Default 3. Also used as the cutoff for the robust modified z-score (median/MAD) method." },
                    "iqr_k": { "type": "number", "minimum": 0, "default": 1.5, "description": "IQR multiplier for Tukey's fences: values below Q1−k·IQR or above Q3+k·IQR are flagged. Default 1.5." }
                },
                "required": ["numbers"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
