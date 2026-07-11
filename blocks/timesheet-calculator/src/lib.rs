//! gizza-ai/timesheet-calculator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute — no host
//! capabilities used.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    log: String,
    #[serde(default)]
    rate: f64,
    #[serde(default)]
    rates: String,
    #[serde(default)]
    currency: String,
    #[serde(default)]
    round: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("log").required().describe(
            "The work log, one entry per line: `[YYYY-MM-DD] START-END PROJECT [notes]`, \
             e.g. `9:00-12:30 Acme kickoff call` or `2024-01-15 13:00-17:15 #Beta`. \
             Times are HH:MM 24-hour or 12-hour with am/pm (`9am`, `5:30pm`); if the \
             end is earlier than the start the entry rolls past midnight (`10pm-2am`). \
             The token after the time range is the project/tag (a leading `#` is \
             stripped); anything after it is notes. Blank lines and lines starting \
             with `#` or `//` are ignored.",
        ))
        .param(
            Param::number("rate")
                .min(0.0)
                .default(0.0)
                .describe(
                    "Fallback hourly billing rate applied to every project (default 0 = \
                     hours only, no money). Override individual projects with `rates`.",
                ),
        )
        .param(Param::string("rates").describe(
            "Optional per-project hourly rate overrides as `Project=amount` pairs, \
             separated by commas or newlines, e.g. `Acme=150, Beta=90`. A project not \
             listed here uses `rate`.",
        ))
        .param(
            Param::string("currency")
                .default("$")
                .describe("Currency symbol/prefix for amounts (default `$`)."),
        )
        .param(
            Param::enumv("round", ["0", "6", "10", "15", "30", "60"])
                .default("0")
                .describe(
                    "Billing increment in minutes — each entry's duration is rounded to \
                     the nearest multiple. `0` = exact (no rounding); `6` = tenths of an \
                     hour (the legal-billing standard); `15`/`30`/`60` for payroll.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn round_to_i64(s: &str) -> i64 {
    match s.trim() {
        "" => 0,
        v => v.parse().unwrap_or(0),
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/timesheet-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Total work-log hours per project and compute billable amounts",
    skill(
        description = "Parse a freeform or structured work log of start–stop times tagged by project, then total the hours per project and compute billable amounts. Each line is `[YYYY-MM-DD] START-END PROJECT [notes]` (24-hour or am/pm times; overnight ranges like 10pm-2am roll past midnight). Set an hourly `rate` (with optional per-project `rates` overrides) to get money totals, and `round` durations to a billing increment (6 minutes = 0.1h legal standard). Returns per-entry breakdowns, per-project rollups, and grand totals of minutes, hours and amount. Ideal for freelancers, consultants and payroll.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "timesheet-calculator", |a: Args| {
            gizza_ai_timesheet_calculator_core::compute(
                &a.log,
                a.rate,
                &a.rates,
                &a.currency,
                round_to_i64(&a.round),
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
                    "log": { "type": "string", "description": "The work log, one entry per line: `[YYYY-MM-DD] START-END PROJECT [notes]`, e.g. `9:00-12:30 Acme kickoff call` or `2024-01-15 13:00-17:15 #Beta`. Times are HH:MM 24-hour or 12-hour with am/pm (`9am`, `5:30pm`); if the end is earlier than the start the entry rolls past midnight (`10pm-2am`). The token after the time range is the project/tag (a leading `#` is stripped); anything after it is notes. Blank lines and lines starting with `#` or `//` are ignored." },
                    "rate": { "type": "number", "minimum": 0, "default": 0.0, "description": "Fallback hourly billing rate applied to every project (default 0 = hours only, no money). Override individual projects with `rates`." },
                    "rates": { "type": "string", "description": "Optional per-project hourly rate overrides as `Project=amount` pairs, separated by commas or newlines, e.g. `Acme=150, Beta=90`. A project not listed here uses `rate`." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol/prefix for amounts (default `$`)." },
                    "round": { "type": "string", "enum": ["0", "6", "10", "15", "30", "60"], "default": "0", "description": "Billing increment in minutes — each entry's duration is rounded to the nearest multiple. `0` = exact (no rounding); `6` = tenths of an hour (the legal-billing standard); `15`/`30`/`60` for payroll." }
                },
                "required": ["log"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
