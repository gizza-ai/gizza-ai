//! gizza-ai/macd-calculator — MACD (Moving Average Convergence Divergence) over a
//! numeric price series. Thin chat-skill wrapper; the chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_macd_calculator_core::{compute, MAX_PERIOD};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    prices: String,
    #[serde(default = "default_fast")]
    fast: u32,
    #[serde(default = "default_slow")]
    slow: u32,
    #[serde(default = "default_signal")]
    signal: u32,
}

fn default_fast() -> u32 {
    12
}
fn default_slow() -> u32 {
    26
}
fn default_signal() -> u32 {
    9
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("prices")
                .required()
                .describe("The numeric price series, separated by spaces, commas, semicolons, or newlines (oldest first)."),
        )
        .param(
            Param::integer("fast")
                .default(12)
                .min(1.0)
                .max(MAX_PERIOD as f64)
                .describe("Fast EMA period (default 12). Must be smaller than the slow period."),
        )
        .param(
            Param::integer("slow")
                .default(26)
                .min(1.0)
                .max(MAX_PERIOD as f64)
                .describe("Slow EMA period (default 26). Must not exceed the number of data points."),
        )
        .param(
            Param::integer("signal")
                .default(9)
                .min(1.0)
                .max(MAX_PERIOD as f64)
                .describe("Signal EMA period — the EMA of the MACD line (default 9)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/macd-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Computes the MACD line, signal line and histogram from prices",
    skill(
        description = "Compute the MACD (Moving Average Convergence Divergence) momentum indicator over a numeric price series (separated by spaces, commas, semicolons, or newlines, oldest first). Uses a fast EMA (default period 12) and a slow EMA (default 26): the MACD line = fastEMA - slowEMA, the signal line = EMA (default 9) of the MACD line, and the histogram = MACD line - signal line. The fast/slow/signal periods are configurable (each 1-10000; fast must be smaller than slow, and slow must not exceed the number of data points). Each EMA is seeded with the simple mean of its first window. Returns the count, the periods used, the fast and slow EMA, the MACD/signal/histogram value at each point (null during the warm-up before enough points are available), and the latest MACD, signal and histogram values. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "macd-calculator", |a: Args| {
            compute(&a.prices, a.fast, a.slow, a.signal).map_err(SkillError::InvalidArgs)
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
                    "prices": { "type": "string", "description": "The numeric price series, separated by spaces, commas, semicolons, or newlines (oldest first)." },
                    "fast": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 12, "description": "Fast EMA period (default 12). Must be smaller than the slow period." },
                    "slow": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 26, "description": "Slow EMA period (default 26). Must not exceed the number of data points." },
                    "signal": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 9, "description": "Signal EMA period — the EMA of the MACD line (default 9)." }
                },
                "required": ["prices"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
