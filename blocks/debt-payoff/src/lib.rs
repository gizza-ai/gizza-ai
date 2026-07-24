//! gizza-ai/debt-payoff — chat skill block on the shared tool abstraction.
//!
//! Builds a debt-payoff plan from a list of debts (name, balance, APR%, minimum
//! payment), a method (snowball = smallest balance first, avalanche = highest
//! APR first), a constant extra monthly payment, and a start date. Uses the
//! standard rollover method (freed-up minimums cascade onto the next debt).
//!
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); `handle()` delegates to `block_utils::run_skill`, which wraps the
//! returned `PlanResult` in `{ "result": … }`. Pure compute — the only host
//! capability used is the clock (`chrono::Utc::now`) to resolve the default
//! start date ("today") when the caller omits one.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    debts: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    extra_payment: f64,
    #[serde(default)]
    start_date: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("debts").required().describe(
                "Your debts, one per line as `name, balance, APR%, minimum payment` — e.g. \
                 `Visa, 2500, 19.99, 75`. Balance and minimum are dollar amounts; APR is the \
                 yearly interest rate as a percent. `$` and `%` symbols are ignored; do NOT use \
                 thousands separators (the comma splits the fields). Up to 50 debts.",
            ),
        )
        .param(
            Param::enumv("method", ["snowball", "avalanche"])
                .default("snowball")
                .describe(
                    "Payoff strategy. 'snowball' targets the smallest balance first (fastest wins \
                     for motivation); 'avalanche' targets the highest APR first (least total \
                     interest). Both roll a cleared debt's payment onto the next one. Default \
                     snowball.",
                ),
        )
        .param(
            Param::number("extra_payment")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Extra dollars you can pay every month on top of all the minimums. This is \
                     added to the target debt and cascades as debts are cleared. Default 0.",
                ),
        )
        .param(
            Param::string("start_date").describe(
                "The month the plan starts, as YYYY-MM-DD (e.g. 2026-01-01). Drives the debt-free \
                 date and each debt's payoff date. Defaults to today when omitted.",
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
    name = "gizza-ai/debt-payoff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Plan a debt payoff with the snowball or avalanche method",
    skill(
        description = "Build a debt-payoff plan from a list of debts (name, balance, APR%, minimum payment). Choose the snowball method (smallest balance first) or avalanche (highest APR first); both use the rollover method where a cleared debt's payment cascades onto the next one. Add a constant extra monthly payment and a start date. Returns the chosen plan (payoff order with per-debt interest, total paid, months and payoff date), total months, debt-free date, total interest and total paid, a minimum-only baseline with interest and months saved, and a snowball-vs-avalanche comparison with a recommendation. Returns an actionable error when the budget can never clear the debts or the input is invalid.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "debt-payoff", |a: Args| {
            gizza_ai_debt_payoff_core::plan(
                &a.debts,
                &a.method,
                a.extra_payment,
                &a.start_date,
                today_utc(),
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-07-23 for the initial debt-payoff release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "debts": { "type": "string", "description": "Your debts, one per line as `name, balance, APR%, minimum payment` — e.g. `Visa, 2500, 19.99, 75`. Balance and minimum are dollar amounts; APR is the yearly interest rate as a percent. `$` and `%` symbols are ignored; do NOT use thousands separators (the comma splits the fields). Up to 50 debts." },
                    "method": { "type": "string", "enum": ["snowball", "avalanche"], "default": "snowball", "description": "Payoff strategy. 'snowball' targets the smallest balance first (fastest wins for motivation); 'avalanche' targets the highest APR first (least total interest). Both roll a cleared debt's payment onto the next one. Default snowball." },
                    "extra_payment": { "type": "number", "default": 0.0, "minimum": 0, "description": "Extra dollars you can pay every month on top of all the minimums. This is added to the target debt and cascades as debts are cleared. Default 0." },
                    "start_date": { "type": "string", "description": "The month the plan starts, as YYYY-MM-DD (e.g. 2026-01-01). Drives the debt-free date and each debt's payoff date. Defaults to today when omitted." }
                },
                "required": ["debts"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
