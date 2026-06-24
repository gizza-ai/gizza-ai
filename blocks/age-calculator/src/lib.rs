//! gizza-ai/age-calculator — chat skill block on the shared tool abstraction.
//!
//! Computes a person's exact age from a birthdate, as of a target date (or
//! today if omitted). The chat schema is single-sourced from `descriptor()`
//! (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`, which shapes `{ "result": <Age> }` so the LLM sees
//! the full structured breakdown (years/months/days + totals + next birthday +
//! zodiac + a human-readable `summary`). Pure compute — the only host capability
//! used is the clock (`chrono::Utc::now`) to resolve "today" when the caller
//! does not pass a target date.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    birthdate: String,
    #[serde(default)]
    as_of: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("birthdate").required().describe(
                "The date of birth. Accepts YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, \
                 RFC-3339 (with Z/offset), and common variants like YYYY/MM/DD, \
                 MM/DD/YYYY, DD.MM.YYYY.",
            ),
        )
        .param(
            Param::string("as_of").describe(
                "Optional 'as of' date to compute the age at, in the same \
                 accepted formats as `birthdate`. Defaults to today (UTC) when \
                 omitted.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn today_utc() -> chrono::NaiveDate {
    use chrono::Datelike;
    let now = chrono::Utc::now();
    chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()).unwrap()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/age-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Exact age in years, months, and days",
    skill(
        description = "Calculate a person's exact age from their birthdate as of a target date (or today if omitted). Returns a calendar breakdown (years, months, days), flat totals (total months, weeks, days, hours), the weekday they were born on, the date and countdown to their next birthday, the Western zodiac sign, and a human-readable summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "age-calculator", |a: Args| {
            gizza_ai_age_calculator_core::age(&a.birthdate, &a.as_of, today_utc())
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
                    "birthdate": { "type": "string", "description": "The date of birth. Accepts YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, RFC-3339 (with Z/offset), and common variants like YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY." },
                    "as_of": { "type": "string", "description": "Optional 'as of' date to compute the age at, in the same accepted formats as `birthdate`. Defaults to today (UTC) when omitted." }
                },
                "required": ["birthdate"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
