//! gizza-ai/salary-convert — chat skill block on the shared tool abstraction.
//!
//! Converts a single pay figure between hourly, daily, weekly, biweekly, monthly
//! and annual rates, pivoting through an annual salary derived from an explicit
//! work schedule (hours/week, days/week, weeks/year). The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`, which shapes `{ "result": <Breakdown> }`
//! so the LLM sees every period figure at once. Pure compute — no host capabilities.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_period() -> String {
    "annual".to_string()
}
fn default_hours() -> f64 {
    40.0
}
fn default_days() -> f64 {
    5.0
}
fn default_weeks() -> f64 {
    52.0
}
fn default_currency() -> String {
    "$".to_string()
}

#[derive(Deserialize)]
struct Args {
    amount: f64,
    #[serde(default = "default_period")]
    period: String,
    #[serde(default = "default_hours")]
    hours_per_week: f64,
    #[serde(default = "default_days")]
    days_per_week: f64,
    #[serde(default = "default_weeks")]
    weeks_per_year: f64,
    #[serde(default = "default_currency")]
    currency: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("amount").required().min(0.0).describe(
                "The pay figure to convert, as a plain number (no currency symbol or thousands \
                 separators) — e.g. 25 for $25/hour or 52000 for a $52,000 salary. This is the \
                 amount for the `period` you choose below.",
            ),
        )
        .param(
            Param::enumv(
                "period",
                ["hourly", "daily", "weekly", "biweekly", "monthly", "annual"],
            )
            .default("annual")
            .describe(
                "Which pay period `amount` is given in. 'annual' (default) treats it as a yearly \
                 salary; 'hourly' as an hourly wage; 'biweekly' is every two weeks; the result \
                 always shows ALL six figures. Aliases like hr/day/week/month/year are accepted.",
            ),
        )
        .param(
            Param::number("hours_per_week")
                .default(40.0)
                .min(0.0)
                .max(168.0)
                .describe(
                    "Hours worked per week, used for the hourly figure. Default 40. Use 20 for \
                     half-time, 37.5 for a 7.5-hour day, etc.",
                ),
        )
        .param(
            Param::number("days_per_week")
                .default(5.0)
                .min(0.0)
                .max(7.0)
                .describe(
                    "Days worked per week, used for the daily figure. Default 5 (Mon–Fri).",
                ),
        )
        .param(
            Param::number("weeks_per_year")
                .default(52.0)
                .min(0.0)
                .max(53.0)
                .describe(
                    "Paid weeks worked per year. Default 52. Lower it for unpaid time off — e.g. \
                     50 for two unpaid weeks.",
                ),
        )
        .param(
            Param::string("currency").default("$").describe(
                "Currency symbol shown in the summary text only (e.g. $, £, €). Does not change \
                 the numeric figures. Default '$'.",
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
    name = "gizza-ai/salary-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert pay between hourly, daily, weekly, biweekly, monthly and annual",
    skill(
        description = "Convert a pay figure between hourly, daily, weekly, biweekly, monthly and annual rates. Give the amount and which period it's in (e.g. 25 hourly, or 52000 annual), plus an optional work schedule (hours per week, days per week, weeks per year — defaults 40/5/52), and get ALL six equivalent figures back at once. Use it to turn an hourly wage into a yearly salary, a monthly salary into an hourly rate, compare job offers quoted in different periods, or price contract/freelance day rates.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "salary-convert", |a: Args| {
            gizza_ai_salary_convert_core::convert(
                a.amount,
                &a.period,
                a.hours_per_week,
                a.days_per_week,
                a.weeks_per_year,
                &a.currency,
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

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored manifest schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "amount": { "type": "number", "minimum": 0, "description": "The pay figure to convert, as a plain number (no currency symbol or thousands separators) — e.g. 25 for $25/hour or 52000 for a $52,000 salary. This is the amount for the `period` you choose below." },
                    "period": { "type": "string", "enum": ["hourly", "daily", "weekly", "biweekly", "monthly", "annual"], "default": "annual", "description": "Which pay period `amount` is given in. 'annual' (default) treats it as a yearly salary; 'hourly' as an hourly wage; 'biweekly' is every two weeks; the result always shows ALL six figures. Aliases like hr/day/week/month/year are accepted." },
                    "hours_per_week": { "type": "number", "default": 40.0, "minimum": 0, "maximum": 168, "description": "Hours worked per week, used for the hourly figure. Default 40. Use 20 for half-time, 37.5 for a 7.5-hour day, etc." },
                    "days_per_week": { "type": "number", "default": 5.0, "minimum": 0, "maximum": 7, "description": "Days worked per week, used for the daily figure. Default 5 (Mon–Fri)." },
                    "weeks_per_year": { "type": "number", "default": 52.0, "minimum": 0, "maximum": 53, "description": "Paid weeks worked per year. Default 52. Lower it for unpaid time off — e.g. 50 for two unpaid weeks." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol shown in the summary text only (e.g. $, £, €). Does not change the numeric figures. Default '$'." }
                },
                "required": ["amount"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
