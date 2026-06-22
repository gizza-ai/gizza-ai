//! gizza-ai/moving-average — simple (SMA) and exponential (EMA) moving averages
//! over a numeric series. Thin chat-skill wrapper; the chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_moving_average_core::{compute, MAX_PERIOD};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    series: String,
    /// Look-back window; the core requires 1..=MAX_PERIOD and <= series length.
    #[serde(default = "default_period")]
    period: u32,
}

fn default_period() -> u32 {
    3
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("series")
                .required()
                .describe("The numeric series (e.g. prices) to average, separated by spaces, commas, semicolons, or newlines."),
        )
        .param(
            Param::integer("period")
                .default(3)
                .min(1.0)
                .max(MAX_PERIOD as f64)
                .describe("Look-back window size for the moving average, 1-10000 (default 3). Must not exceed the number of data points."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MovingAverage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/moving-average",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Simple, exponential, and weighted moving averages over a numeric series",
    skill(
        description = "Compute simple (SMA), exponential (EMA), and weighted (WMA) moving averages over a numeric series (e.g. stock/price data), separated by spaces, commas, semicolons, or newlines. 'period' (default 3) is the look-back window; it must be between 1 and 10000 and no larger than the number of data points. Returns the count, period, EMA smoothing factor (2/(period+1)), and the SMA, EMA, and WMA value at each point (null during the warm-up before 'period' points are available). The EMA is seeded with the simple mean of the first window; the WMA uses linear weights with the most recent value weighted heaviest. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl MovingAverage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "moving-average", |a: Args| {
            compute(&a.series, a.period).map_err(SkillError::InvalidArgs)
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
                    "series": { "type": "string", "description": "The numeric series (e.g. prices) to average, separated by spaces, commas, semicolons, or newlines." },
                    "period": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 3, "description": "Look-back window size for the moving average, 1-10000 (default 3). Must not exceed the number of data points." }
                },
                "required": ["series"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
