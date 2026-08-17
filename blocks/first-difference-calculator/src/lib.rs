//! gizza-ai/first-difference-calculator — first (and higher-order) differences
//! of a numeric series, in absolute, percent, ratio, or log form. Thin chat-skill
//! wrapper; the chat schema is single-sourced from descriptor() (which also drives
//! the CLI); handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_first_difference_calculator_core::{compute, MAX_DECIMALS, MAX_LAG, MAX_ORDER};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    series: String,
    #[serde(default = "default_lag")]
    lag: i64,
    #[serde(default = "default_order")]
    order: u32,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_decimals")]
    decimals: u32,
    #[serde(default)]
    drop_warmup: bool,
}

fn default_lag() -> i64 {
    1
}
fn default_order() -> u32 {
    1
}
fn default_mode() -> String {
    "difference".to_string()
}
fn default_decimals() -> u32 {
    6
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("series")
                .required()
                .describe("The numeric series to difference, in order, separated by spaces, commas, semicolons, or newlines (e.g. '120, 135, 150, 148'). Plain decimals or scientific notation only — strip currency symbols and thousands separators first. Needs at least 2 values; max 20,000."),
        )
        .param(
            Param::integer("lag")
                .default(1)
                .min(-(MAX_LAG as f64))
                .max(MAX_LAG as f64)
                .describe("How far back the baseline sits, -1000 to 1000, never 0 (default 1). 1 compares each value with the one before it; 12 gives seasonal differencing for monthly data; a NEGATIVE lag compares each value with a LATER one, which moves the warm-up rows to the end."),
        )
        .param(
            Param::integer("order")
                .default(1)
                .min(1.0)
                .max(MAX_ORDER as f64)
                .describe("How many times to repeat the transform on its own output, 1-10 (default 1). 2 gives second differences; each extra order adds |lag| more warm-up rows."),
        )
        .param(
            Param::enumv("mode", ["difference", "percent", "ratio", "log"])
                .default("difference")
                .describe("How each value is compared with its baseline: 'difference' (default) is current - baseline; 'percent' is (current - baseline) / baseline x 100; 'ratio' is current / baseline; 'log' is ln(current / baseline). A zero baseline (or a non-positive value in log mode) returns null instead of infinity."),
        )
        .param(
            Param::integer("decimals")
                .default(6)
                .min(0.0)
                .max(MAX_DECIMALS as f64)
                .describe("Decimal places to round every returned number to, 0-10 (default 6)."),
        )
        .param(
            Param::boolean("drop_warmup")
                .default(false)
                .describe("Drop the warm-up rows that have no baseline instead of returning them as aligned nulls (default false). False keeps the output the same length as the input so row i still lines up with input row i; true gives the shorter n - |lag| x order form."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FirstDifferenceCalculator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/first-difference-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "First differences and period-over-period deltas of a numeric series",
    skill(
        description = "Compute first differences (or period-over-period deltas) of a numeric series, separated by spaces, commas, semicolons, or newlines. 'lag' (default 1, -1000..1000, never 0) sets how far back the baseline sits — 1 for step-to-step change, 12 for seasonal differencing, a negative value to compare with a later point. 'order' (default 1, max 10) repeats the transform for second/third differences. 'mode' picks difference (current - baseline), percent ((current - baseline) / baseline x 100), ratio (current / baseline), or log (ln(current / baseline)). 'decimals' (0-10, default 6) rounds the output; 'drop_warmup' (default false) trims the rows with no baseline instead of returning aligned nulls. Returns the per-row values plus their original indices, counts of increases/decreases/unchanged, min/max/mean/sum, the largest move and its index, a constant-differences flag, and a plain-language interpretation (constant first differences mean a linear series, constant second differences a quadratic one). Zero baselines and non-positive log inputs come back as null and are counted, never as infinity. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl FirstDifferenceCalculator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "first-difference-calculator", |a: Args| {
            compute(
                &a.series,
                a.lag,
                a.order,
                &a.mode,
                a.decimals,
                a.drop_warmup,
            )
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "series": {
                        "type": "string",
                        "description": "The numeric series to difference, in order, separated by spaces, commas, semicolons, or newlines (e.g. '120, 135, 150, 148'). Plain decimals or scientific notation only — strip currency symbols and thousands separators first. Needs at least 2 values; max 20,000."
                    },
                    "lag": {
                        "type": "integer",
                        "minimum": -1000,
                        "maximum": 1000,
                        "default": 1,
                        "description": "How far back the baseline sits, -1000 to 1000, never 0 (default 1). 1 compares each value with the one before it; 12 gives seasonal differencing for monthly data; a NEGATIVE lag compares each value with a LATER one, which moves the warm-up rows to the end."
                    },
                    "order": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10,
                        "default": 1,
                        "description": "How many times to repeat the transform on its own output, 1-10 (default 1). 2 gives second differences; each extra order adds |lag| more warm-up rows."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["difference", "percent", "ratio", "log"],
                        "default": "difference",
                        "description": "How each value is compared with its baseline: 'difference' (default) is current - baseline; 'percent' is (current - baseline) / baseline x 100; 'ratio' is current / baseline; 'log' is ln(current / baseline). A zero baseline (or a non-positive value in log mode) returns null instead of infinity."
                    },
                    "decimals": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 10,
                        "default": 6,
                        "description": "Decimal places to round every returned number to, 0-10 (default 6)."
                    },
                    "drop_warmup": {
                        "type": "boolean",
                        "default": false,
                        "description": "Drop the warm-up rows that have no baseline instead of returning them as aligned nulls (default false). False keeps the output the same length as the input so row i still lines up with input row i; true gives the shorter n - |lag| x order form."
                    }
                },
                "required": ["series"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_default_to_a_plain_first_difference() {
        let a: Args = serde_json::from_str(r#"{"series":"1 4 7"}"#).unwrap();
        assert_eq!(a.lag, 1);
        assert_eq!(a.order, 1);
        assert_eq!(a.mode, "difference");
        assert_eq!(a.decimals, 6);
        assert!(!a.drop_warmup);
        let d = compute(&a.series, a.lag, a.order, &a.mode, a.decimals, a.drop_warmup).unwrap();
        assert_eq!(d.values, vec![None, Some(3.0), Some(3.0)]);
        assert!(d.summary.constant);
    }
}
