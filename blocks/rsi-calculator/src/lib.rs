//! gizza-ai/rsi-calculator — RSI (Relative Strength Index) over a numeric price
//! series. Thin chat-skill wrapper; the chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rsi_calculator_core::{compute, MAX_PERIOD};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    prices: String,
    #[serde(default = "default_period")]
    period: u32,
    #[serde(default = "default_overbought")]
    overbought: f64,
    #[serde(default = "default_oversold")]
    oversold: f64,
}

fn default_period() -> u32 {
    14
}
fn default_overbought() -> f64 {
    70.0
}
fn default_oversold() -> f64 {
    30.0
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
            Param::integer("period")
                .default(14)
                .min(1.0)
                .max(MAX_PERIOD as f64)
                .describe("RSI look-back period (default 14). Needs at least period + 1 data points."),
        )
        .param(
            Param::number("overbought")
                .default(70.0)
                .min(0.0)
                .max(100.0)
                .describe("Overbought threshold for classifying the latest reading (default 70)."),
        )
        .param(
            Param::number("oversold")
                .default(30.0)
                .min(0.0)
                .max(100.0)
                .describe("Oversold threshold for classifying the latest reading (default 30)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rsi-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Computes the Relative Strength Index (RSI) from a price series",
    skill(
        description = "Compute the Relative Strength Index (RSI) — Wilder's momentum oscillator — over a numeric price series (separated by spaces, commas, semicolons, or newlines, oldest first). For each look-back window (default period 14) it splits the period-over-period price changes into average gains and average losses using Wilder's smoothing, then reports RSI = 100 - 100/(1 + avgGain/avgLoss) on a 0-100 scale. RSI is 100 when the window has no losses and 0 when it has no gains. Returns the count, the period and overbought/oversold thresholds used, the RSI value and Wilder-smoothed average gain/loss at each point (null during the warm-up before period+1 points are available), the latest RSI, and a latest-signal classification (overbought/oversold/neutral). The period (1-10000) and the overbought (default 70) and oversold (default 30) thresholds are configurable. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rsi-calculator", |a: Args| {
            compute(&a.prices, a.period, a.overbought, a.oversold).map_err(SkillError::InvalidArgs)
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
                    "period": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 14, "description": "RSI look-back period (default 14). Needs at least period + 1 data points." },
                    "overbought": { "type": "number", "minimum": 0, "maximum": 100, "default": 70.0, "description": "Overbought threshold for classifying the latest reading (default 70)." },
                    "oversold": { "type": "number", "minimum": 0, "maximum": 100, "default": 30.0, "description": "Oversold threshold for classifying the latest reading (default 30)." }
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
