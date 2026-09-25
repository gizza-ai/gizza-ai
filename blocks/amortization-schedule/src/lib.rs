//! gizza-ai/amortization-schedule — full fixed-rate loan amortization schedules.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI and generated page manifest); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_amortization_schedule_core::Inputs;
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    loan_amount: Option<f64>,
    #[serde(default)]
    annual_interest_rate_percent: Option<f64>,
    #[serde(default)]
    loan_years: Option<f64>,
    #[serde(default)]
    loan_months: Option<f64>,
    #[serde(default)]
    payment_frequency: Option<String>,
    #[serde(default)]
    start_date: Option<String>,
    #[serde(default)]
    extra_payment: Option<f64>,
    #[serde(default)]
    extra_one_time: Option<f64>,
    #[serde(default)]
    extra_one_time_period: Option<f64>,
    #[serde(default)]
    schedule_view: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    currency_symbol: Option<String>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            loan_amount: self.loan_amount,
            annual_interest_rate_percent: self.annual_interest_rate_percent,
            loan_years: self.loan_years,
            loan_months: self.loan_months,
            payment_frequency: self.payment_frequency,
            start_date: self.start_date,
            extra_payment: self.extra_payment,
            extra_one_time: self.extra_one_time,
            extra_one_time_period: self.extra_one_time_period,
            schedule_view: self.schedule_view,
            format: self.format,
            currency_symbol: self.currency_symbol,
        }
    }
}

const LOAN_AMOUNT_DESC: &str = "Amount borrowed / principal balance to amortize. Default 300000. Must be greater than 0 and at most 1000000000.";
const RATE_DESC: &str =
    "Nominal annual interest rate as a percent, e.g. 6 for 6% APR. Default 6. Range 0–100.";
const YEARS_DESC: &str = "Whole years in the loan term. Default 30. Combine with loan_months for terms like 5 years 6 months.";
const MONTHS_DESC: &str = "Additional months in the loan term beyond loan_years. Default 0. The total term must be at least one month.";
const FREQUENCY_DESC: &str = "Payment frequency used for the amortization schedule and per-period interest: weekly, biweekly, monthly, quarterly, semiannual, or annual. Default monthly.";
const START_DATE_DESC: &str = "First payment date. Use YYYY-MM-DD (also accepts YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY). Blank uses 2026-01-01 for deterministic output.";
const EXTRA_DESC: &str = "Extra principal paid every period, on top of the scheduled payment. Default 0. It shortens the payoff and reduces interest.";
const LUMP_DESC: &str =
    "One-time extra principal amount applied in extra_one_time_period. Default 0.";
const LUMP_PERIOD_DESC: &str = "1-based payment number where extra_one_time is applied. Default 1. Must be within the scheduled term when a lump sum is entered.";
const VIEW_DESC: &str = "Schedule density: period returns every payment row; annual groups rows by loan year. Default period.";
const FORMAT_DESC: &str = "Output format: table for a readable aligned report, csv for spreadsheet rows, or json for the full structured schedule. Default table.";
const SYMBOL_DESC: &str = "Currency symbol printed in the table summary, up to 3 characters. Default $. Cosmetic only; no conversion is applied.";

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("loan_amount")
                .default(300_000.0)
                .min(0.01)
                .max(1_000_000_000.0)
                .describe(LOAN_AMOUNT_DESC),
        )
        .param(
            Param::number("annual_interest_rate_percent")
                .default(6.0)
                .min(0.0)
                .max(100.0)
                .describe(RATE_DESC),
        )
        .param(
            Param::number("loan_years")
                .default(30.0)
                .min(0.0)
                .max(100.0)
                .describe(YEARS_DESC),
        )
        .param(
            Param::number("loan_months")
                .default(0.0)
                .min(0.0)
                .max(1200.0)
                .describe(MONTHS_DESC),
        )
        .param(
            Param::enumv(
                "payment_frequency",
                [
                    "weekly",
                    "biweekly",
                    "monthly",
                    "quarterly",
                    "semiannual",
                    "annual",
                ],
            )
            .default("monthly")
            .describe(FREQUENCY_DESC),
        )
        .param(
            Param::string("start_date")
                .default("2026-01-01")
                .describe(START_DATE_DESC),
        )
        .param(
            Param::number("extra_payment")
                .default(0.0)
                .min(0.0)
                .max(1_000_000_000.0)
                .describe(EXTRA_DESC),
        )
        .param(
            Param::number("extra_one_time")
                .default(0.0)
                .min(0.0)
                .max(1_000_000_000.0)
                .describe(LUMP_DESC),
        )
        .param(
            Param::integer("extra_one_time_period")
                .default(1)
                .min(1.0)
                .max(5000.0)
                .describe(LUMP_PERIOD_DESC),
        )
        .param(
            Param::enumv("schedule_view", ["period", "annual"])
                .default("period")
                .describe(VIEW_DESC),
        )
        .param(
            Param::enumv("format", ["table", "csv", "json"])
                .default("table")
                .describe(FORMAT_DESC),
        )
        .param(
            Param::string("currency_symbol")
                .default("$")
                .describe(SYMBOL_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/amortization-schedule",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a loan amortization schedule with payments, interest, balances, and totals.",
    skill(
        description = "Generate a fixed-rate loan amortization schedule. Pass loan_amount, annual_interest_rate_percent, loan_years and optional loan_months; choose payment_frequency (weekly, biweekly, monthly, quarterly, semiannual, annual), optional start_date, recurring extra_payment, one-time extra_one_time plus extra_one_time_period, schedule_view (period or annual), format (table, csv, json), and currency_symbol. Returns per-period or annual principal/interest/balance rows plus payment_per_period, total_interest, total_paid, interest_saved and payments_saved.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "amortization-schedule", |a: Args| {
            gizza_ai_amortization_schedule_core::run_inputs(&a.inputs())
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

    #[test]
    fn descriptor_schema_has_expected_controls() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &v["properties"];
        assert_eq!(props["loan_amount"]["default"], 300000.0);
        assert_eq!(props["annual_interest_rate_percent"]["maximum"], 100);
        assert_eq!(props["payment_frequency"]["enum"][0], "weekly");
        assert_eq!(props["payment_frequency"]["enum"][5], "annual");
        assert_eq!(props["schedule_view"]["enum"][1], "annual");
        assert_eq!(props["format"]["enum"][2], "json");
        assert!(props["currency_symbol"]["description"]
            .as_str()
            .unwrap()
            .contains("Currency"));
    }
}
