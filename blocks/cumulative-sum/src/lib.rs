//! gizza-ai/cumulative-sum — running total (prefix sum) of a list of numbers,
//! with optional running average, min, and max.
//!
//! Thin chat-skill wrapper around `gizza-ai-cumulative-sum-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    numbers: String,
    #[serde(default)]
    average: bool,
    #[serde(default)]
    min: bool,
    #[serde(default)]
    max: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("numbers")
                .required()
                .describe("The list of numbers, separated by commas, spaces, or newlines (one per line). Decimals and negatives are allowed."),
        )
        .param(
            Param::boolean("average")
                .describe("Also include a 'running_avg' column — the average (mean) of the values seen up to and including each row."),
        )
        .param(
            Param::boolean("min")
                .describe("Also include a 'running_min' column — the smallest value seen up to and including each row."),
        )
        .param(
            Param::boolean("max")
                .describe("Also include a 'running_max' column — the largest value seen up to and including each row."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CumulativeSum;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cumulative-sum",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Running total (prefix sum) of a list of numbers.",
    skill(
        description = "Compute the running total (prefix sum) of a list of numbers. Pass the values in 'numbers' (separated by commas, spaces, or newlines). The result is a table with one row per input value: the value and the cumulative sum up to that point. Optionally set average=true to add a running-average column (the mean of values seen so far), min=true for a running-minimum column, and max=true for a running-maximum column. Use it for prefix sums, running balances, cumulative totals, and step-by-step accumulation.",
        parameters = schema_json()
    ),
)]
impl CumulativeSum {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cumulative-sum", |a: Args| {
            gizza_ai_cumulative_sum_core::cumulative(&a.numbers, a.average, a.min, a.max)
                .map_err(SkillError::InvalidArgs)
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
                    "numbers": { "type": "string", "description": "The list of numbers, separated by commas, spaces, or newlines (one per line). Decimals and negatives are allowed." },
                    "average": { "type": "boolean", "description": "Also include a 'running_avg' column — the average (mean) of the values seen up to and including each row." },
                    "min": { "type": "boolean", "description": "Also include a 'running_min' column — the smallest value seen up to and including each row." },
                    "max": { "type": "boolean", "description": "Also include a 'running_max' column — the largest value seen up to and including each row." }
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
