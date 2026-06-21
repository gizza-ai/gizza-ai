//! gizza-ai/bollinger-bands — Bollinger Bands for a price/value series.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_bollinger_bands_core::compute;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    prices: String,
    #[serde(default = "default_period")]
    period: f64,
    #[serde(default = "default_num_std")]
    num_std: f64,
}

fn default_period() -> f64 {
    20.0
}
fn default_num_std() -> f64 {
    2.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("prices").required().describe(
                "The price/value series, separated by spaces, commas, semicolons, or newlines (oldest first).",
            ),
        )
        .param(
            Param::integer("period")
                .min(1.0)
                .default(20.0)
                .describe("Moving-average window length (the SMA period). Defaults to 20."),
        )
        .param(
            Param::number("num_std")
                .min(0.0)
                .default(2.0)
                .describe("Standard-deviation multiplier for the upper/lower bands. Defaults to 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BollingerBands;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bollinger-bands",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bollinger Bands (moving average plus/minus standard deviations)",
    skill(
        description = "Compute Bollinger Bands for a price/value series (separated by spaces, commas, semicolons, or newlines, oldest first). For each rolling window of `period` values (default 20) the middle band is the simple moving average and the upper/lower bands are the SMA plus/minus `num_std` (default 2) population standard deviations. Also returns %B and bandwidth for the latest point. Runs locally.",
        parameters = schema_json()
    ),
)]
impl BollingerBands {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bollinger-bands", |a: Args| {
            if a.period < 1.0 || a.period.fract() != 0.0 {
                return Err(SkillError::InvalidArgs("period must be a positive integer".into()));
            }
            compute(&a.prices, a.period as usize, a.num_std).map_err(SkillError::InvalidArgs)
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
                    "prices": { "type": "string", "description": "The price/value series, separated by spaces, commas, semicolons, or newlines (oldest first)." },
                    "period": { "type": "integer", "minimum": 1, "default": 20.0, "description": "Moving-average window length (the SMA period). Defaults to 20." },
                    "num_std": { "type": "number", "minimum": 0, "default": 2.0, "description": "Standard-deviation multiplier for the upper/lower bands. Defaults to 2." }
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
