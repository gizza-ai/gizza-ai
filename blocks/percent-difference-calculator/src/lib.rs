//! gizza-ai/percent-difference-calculator — chat skill block on the shared tool
//! abstraction.
//!
//! Compares two numbers three ways from one pair of inputs: the absolute
//! difference, the symmetric percent difference (`|a - b| / |mean|`), and the
//! directional percent change (`(b - a) / |a|`). The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill` and returns the human-readable report
//! from `core::summary`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_percent_difference_calculator_core::{MAX_DECIMALS, MODES};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    a: f64,
    b: f64,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_decimals")]
    decimals: u32,
}

fn default_mode() -> String {
    "all".to_string()
}

fn default_decimals() -> u32 {
    4
}

/// Single source for the chat schema (and CLI). `a` and `b` are the two values
/// being compared; `mode` picks which block of measures is reported and
/// `decimals` controls display precision only.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("a")
                .required()
                .describe("First value to compare (the baseline for percent change). Any finite number, negatives included."),
        )
        .param(
            Param::number("b")
                .required()
                .describe("Second value to compare (the new value for percent change). Any finite number, negatives included."),
        )
        .param(
            Param::enumv("mode", MODES)
                .default("all")
                .describe(
                    "Which measures to report. all: both blocks (default). difference: \
                     the symmetric percent difference |a - b| / |mean| * 100, which is \
                     order-independent. change: the directional percent change \
                     (b - a) / |a| * 100 in both directions plus the ratio b / a.",
                ),
        )
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(MAX_DECIMALS as f64)
                .default(4)
                .describe(
                    "Decimal places to show in the report, 0 to 10 (default 4). \
                     Display only — the arithmetic is always done at full precision.",
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
    name = "gizza-ai/percent-difference-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Absolute difference, symmetric percent difference, and percent change between two values.",
    skill(
        description = "Compare two numbers and report how far apart they are. Returns the absolute difference |a - b|, the signed difference b - a with its direction, the mean, the symmetric percent difference |a - b| / |mean| * 100 (order-independent — swapping a and b gives the same answer), and the directional percent change (b - a) / |a| * 100 in both directions plus the ratio b / a. Set mode to all (default), difference, or change to pick which measures are reported, and decimals (0-10, default 4) for display precision. Negative values are supported; a measure whose denominator is zero is omitted with an explanation.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "percent-difference-calculator", |a: Args| {
            gizza_ai_percent_difference_calculator_core::summary(a.a, a.b, &a.mode, a.decimals)
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

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "a": { "type": "number", "description": "First value to compare (the baseline for percent change). Any finite number, negatives included." },
                    "b": { "type": "number", "description": "Second value to compare (the new value for percent change). Any finite number, negatives included." },
                    "mode": { "type": "string", "enum": ["all","difference","change"], "default": "all", "description": "Which measures to report. all: both blocks (default). difference: the symmetric percent difference |a - b| / |mean| * 100, which is order-independent. change: the directional percent change (b - a) / |a| * 100 in both directions plus the ratio b / a." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 10, "default": 4, "description": "Decimal places to show in the report, 0 to 10 (default 4). Display only — the arithmetic is always done at full precision." }
                },
                "required": ["a", "b"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Every param must carry a description — it is the only thing the LLM and
    /// the page both read to explain a field.
    #[test]
    fn every_param_is_described() {
        for p in descriptor().params {
            assert!(!p.description.is_empty(), "param {} needs .describe()", p.name);
        }
    }
}
