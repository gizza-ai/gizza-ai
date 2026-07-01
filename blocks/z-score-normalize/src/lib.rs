//! gizza-ai/z-score-normalize — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_z_score_normalize_core::normalize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    numbers: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    sample: bool,
}

fn default_method() -> String {
    "z-score".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("numbers")
                .required()
                .describe("The numbers to normalize, separated by spaces, commas, semicolons, or newlines."),
        )
        .param(
            Param::enumv("method", ["z-score", "min-max", "max-abs", "robust"])
                .default("z-score")
                .describe("'z-score' (default) standardizes to mean 0 and standard deviation 1; 'min-max' rescales linearly into the 0–1 range; 'max-abs' divides by the largest absolute value, mapping into [−1, 1] while preserving sign; 'robust' subtracts the median and divides by the interquartile range (Q3−Q1), resisting outliers."),
        )
        .param(
            Param::boolean("sample")
                .default(false)
                .describe("z-score only: when true use the sample standard deviation (÷N−1); default false uses the population standard deviation (÷N), matching scikit-learn StandardScaler. Ignored for the other methods."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/z-score-normalize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Standardize a list of numbers (z-score, min-max, max-abs, robust)",
    skill(
        description = "Normalize a list of numbers (separated by spaces, commas, semicolons, or newlines). method='z-score' (default) standardizes each value to mean 0 and standard deviation 1 by subtracting the mean and dividing by the standard deviation (population ÷N by default, or sample ÷N−1 when sample=true); method='min-max' linearly rescales values into the 0–1 range; method='max-abs' divides by the largest absolute value, mapping into [−1, 1] while preserving sign; method='robust' subtracts the median and divides by the interquartile range (Q3−Q1) to resist outliers. Returns the transformed values plus the parameters used (mean/std dev, min/max, max abs, or median/IQR). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "z-score-normalize", |a: Args| {
            normalize(&a.numbers, &a.method, a.sample).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "numbers": { "type": "string", "description": "The numbers to normalize, separated by spaces, commas, semicolons, or newlines." },
                    "method": { "type": "string", "enum": ["z-score", "min-max", "max-abs", "robust"], "default": "z-score", "description": "'z-score' (default) standardizes to mean 0 and standard deviation 1; 'min-max' rescales linearly into the 0–1 range; 'max-abs' divides by the largest absolute value, mapping into [−1, 1] while preserving sign; 'robust' subtracts the median and divides by the interquartile range (Q3−Q1), resisting outliers." },
                    "sample": { "type": "boolean", "default": false, "description": "z-score only: when true use the sample standard deviation (÷N−1); default false uses the population standard deviation (÷N), matching scikit-learn StandardScaler. Ignored for the other methods." }
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
