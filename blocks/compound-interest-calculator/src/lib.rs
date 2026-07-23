//! gizza-ai/compound-interest-calculator — chat skill block on the shared tool
//! abstraction.
//!
//! Computes the future value of an initial principal growing with compound
//! interest, plus the future value of optional regular contributions. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` delegates to `block_utils::run_skill` and returns the pretty
//! result JSON from `core::compute_json`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_compound_interest_calculator_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    principal: Option<f64>,
    #[serde(default)]
    annual_rate: Option<f64>,
    #[serde(default)]
    years: Option<f64>,
    #[serde(default)]
    months: Option<f64>,
    #[serde(default)]
    compounding: Option<String>,
    #[serde(default)]
    contribution: Option<f64>,
    #[serde(default)]
    contribution_frequency: Option<String>,
    #[serde(default)]
    contribution_timing: Option<String>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            principal: self.principal,
            annual_rate: self.annual_rate,
            years: self.years,
            months: self.months,
            compounding: self.compounding,
            contribution: self.contribution,
            contribution_frequency: self.contribution_frequency,
            contribution_timing: self.contribution_timing,
        }
    }
}

const COMPOUNDING: [&str; 7] = [
    "annually",
    "semiannually",
    "quarterly",
    "monthly",
    "weekly",
    "daily",
    "continuously",
];

const CONTRIB_FREQ: [&str; 6] = [
    "annually",
    "semiannually",
    "quarterly",
    "monthly",
    "biweekly",
    "weekly",
];

const TIMING: [&str; 2] = ["end", "start"];

/// Single source for the chat schema (and CLI). Every field is optional and
/// falls back to the documented default, so the tool always returns a result.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("principal")
                .default(1000.0)
                .describe("Initial principal / starting balance in your currency. Default 1000."),
        )
        .param(
            Param::number("annual_rate")
                .default(5.0)
                .min(0.0)
                .max(20.0)
                .describe(
                    "Nominal annual interest rate as a percent, e.g. 5 for 5%. Default 5.",
                ),
        )
        .param(
            Param::number("years")
                .default(10.0)
                .min(0.0)
                .max(1000.0)
                .describe("Whole or partial years in the term. Default 10 (max 1000)."),
        )
        .param(
            Param::number("months")
                .default(0.0)
                .min(0.0)
                .describe("Extra months added on top of years. Default 0."),
        )
        .param(
            Param::enumv("compounding", COMPOUNDING)
                .default("monthly")
                .describe(
                    "How often interest compounds: annually, semiannually, quarterly, \
                     monthly, weekly, daily, or continuously. Default monthly.",
                ),
        )
        .param(
            Param::number("contribution")
                .default(0.0)
                .describe(
                    "Regular contribution added each contribution period. Use a negative \
                     number for periodic withdrawals. Default 0 (lump sum only).",
                ),
        )
        .param(
            Param::enumv("contribution_frequency", CONTRIB_FREQ)
                .default("monthly")
                .describe(
                    "How often the contribution is made: annually, semiannually, \
                     quarterly, monthly, biweekly, or weekly. Default monthly.",
                ),
        )
        .param(
            Param::enumv("contribution_timing", TIMING)
                .default("end")
                .describe(
                    "Whether each contribution is made at the end (ordinary annuity) or \
                     start (annuity-due) of the period. Default end.",
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
    name = "gizza-ai/compound-interest-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Future value with compound interest plus optional regular contributions.",
    skill(
        description = "Compute the future value of a lump sum plus optional regular contributions growing with compound interest. Pass principal (starting balance), annual_rate (nominal percent), years (+ optional months) for the term, compounding (annually/semiannually/quarterly/monthly/weekly/daily/continuously), and optionally contribution + contribution_frequency (annually/semiannually/quarterly/monthly/biweekly/weekly) + contribution_timing (end or start of period). Every parameter is optional with a sensible default. Returns future_value, principal, total_contributions, total_interest, effective_annual_rate (APY), years_to_double, a per-year schedule, and a plain-language summary. Contributions may be negative to model withdrawals.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "compound-interest-calculator", |a: Args| {
            gizza_ai_compound_interest_calculator_core::compute_json(&a.inputs())
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

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "principal": { "type": "number", "default": 1000.0, "description": "Initial principal / starting balance in your currency. Default 1000." },
                    "annual_rate": { "type": "number", "minimum": 0, "maximum": 20, "default": 5.0, "description": "Nominal annual interest rate as a percent, e.g. 5 for 5%. Default 5." },
                    "years": { "type": "number", "minimum": 0, "maximum": 1000, "default": 10.0, "description": "Whole or partial years in the term. Default 10 (max 1000)." },
                    "months": { "type": "number", "minimum": 0, "default": 0.0, "description": "Extra months added on top of years. Default 0." },
                    "compounding": { "type": "string", "enum": ["annually","semiannually","quarterly","monthly","weekly","daily","continuously"], "default": "monthly", "description": "How often interest compounds: annually, semiannually, quarterly, monthly, weekly, daily, or continuously. Default monthly." },
                    "contribution": { "type": "number", "default": 0.0, "description": "Regular contribution added each contribution period. Use a negative number for periodic withdrawals. Default 0 (lump sum only)." },
                    "contribution_frequency": { "type": "string", "enum": ["annually","semiannually","quarterly","monthly","biweekly","weekly"], "default": "monthly", "description": "How often the contribution is made: annually, semiannually, quarterly, monthly, biweekly, or weekly. Default monthly." },
                    "contribution_timing": { "type": "string", "enum": ["end","start"], "default": "end", "description": "Whether each contribution is made at the end (ordinary annuity) or start (annuity-due) of the period. Default end." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
