//! gizza-ai/period-predictor — chat skill block on the shared tool abstraction.
//!
//! Predicts upcoming menstrual periods from the first day of the most recent
//! period plus an average cycle length, bleeding duration, luteal-phase length
//! and a number of cycles to project. For each predicted cycle it returns the
//! period start (with weekday), the bleeding-end date, the estimated ovulation
//! day (period start − luteal phase) and the 6-day fertile window. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` delegates to `block_utils::run_skill`. Pure compute — no host
//! capabilities used. Predictions are estimates only, not a contraceptive
//! method or medical advice.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    last_period: String,
    #[serde(default = "default_cycle_length")]
    cycle_length: i64,
    #[serde(default = "default_period_length")]
    period_length: i64,
    #[serde(default = "default_luteal_phase")]
    luteal_phase: i64,
    #[serde(default = "default_cycles")]
    cycles: i64,
}

fn default_cycle_length() -> i64 {
    28
}
fn default_period_length() -> i64 {
    5
}
fn default_luteal_phase() -> i64 {
    14
}
fn default_cycles() -> i64 {
    6
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("last_period").required().describe(
                "The first day of the most recent period. Accepts YYYY-MM-DD, \
                 YYYY-MM-DDTHH:MM:SS, RFC-3339 (with Z/offset), and common \
                 variants like YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY.",
            ),
        )
        .param(
            Param::integer("cycle_length")
                .default(28)
                .min(20.0)
                .max(45.0)
                .describe(
                    "Average cycle length in days — first day of one period to \
                     the first day of the next. Default 28. Accepts 20–45.",
                ),
        )
        .param(
            Param::integer("period_length")
                .default(5)
                .min(1.0)
                .max(14.0)
                .describe(
                    "How many days the bleeding lasts, used for each cycle's \
                     end date. Default 5. Accepts 1–14.",
                ),
        )
        .param(
            Param::integer("luteal_phase")
                .default(14)
                .min(9.0)
                .max(17.0)
                .describe(
                    "Luteal-phase length in days, used to estimate ovulation \
                     (period start − luteal phase). Default 14. Accepts 9–17.",
                ),
        )
        .param(
            Param::integer("cycles")
                .default(6)
                .min(1.0)
                .max(24.0)
                .describe("How many upcoming cycles to predict. Default 6. Accepts 1–24."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/period-predictor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Predict upcoming periods, ovulation and fertile windows",
    skill(
        description = "Predict a person's upcoming menstrual periods from the first day of their most recent period. cycle_length is the average days between period starts (default 28), period_length is how many days bleeding lasts (default 5), luteal_phase is the days before a period that ovulation happens (default 14), and cycles is how many future cycles to project (default 6). Returns each predicted cycle's period start date and weekday, bleeding-end date, estimated ovulation day (period start − luteal phase) and 6-day fertile window (five days before ovulation through ovulation day), plus the next period start and a human-readable summary. Estimates only — not a contraceptive method or medical advice. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "period-predictor", |a: Args| {
            gizza_ai_period_predictor_core::period_predict(
                &a.last_period,
                a.cycle_length,
                a.period_length,
                a.luteal_phase,
                a.cycles,
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
                    "last_period": { "type": "string", "description": "The first day of the most recent period. Accepts YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, RFC-3339 (with Z/offset), and common variants like YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY." },
                    "cycle_length": { "type": "integer", "minimum": 20, "maximum": 45, "default": 28, "description": "Average cycle length in days — first day of one period to the first day of the next. Default 28. Accepts 20–45." },
                    "period_length": { "type": "integer", "minimum": 1, "maximum": 14, "default": 5, "description": "How many days the bleeding lasts, used for each cycle's end date. Default 5. Accepts 1–14." },
                    "luteal_phase": { "type": "integer", "minimum": 9, "maximum": 17, "default": 14, "description": "Luteal-phase length in days, used to estimate ovulation (period start − luteal phase). Default 14. Accepts 9–17." },
                    "cycles": { "type": "integer", "minimum": 1, "maximum": 24, "default": 6, "description": "How many upcoming cycles to predict. Default 6. Accepts 1–24." }
                },
                "required": ["last_period"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
