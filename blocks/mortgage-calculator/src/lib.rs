//! gizza-ai/mortgage-calculator — chat skill block on the shared tool
//! abstraction.
//!
//! Computes the fixed-rate monthly mortgage payment (principal, interest, taxes,
//! insurance, HOA) and the full cost of the loan, including the effect of an
//! extra monthly principal payment on the payoff term. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill` and returns the pretty result JSON from
//! `core::compute_json`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mortgage_calculator_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    home_price: Option<f64>,
    #[serde(default)]
    down_payment: Option<f64>,
    #[serde(default)]
    loan_years: Option<f64>,
    #[serde(default)]
    annual_interest_rate_percent: Option<f64>,
    #[serde(default)]
    annual_property_tax: Option<f64>,
    #[serde(default)]
    annual_insurance: Option<f64>,
    #[serde(default)]
    monthly_hoa: Option<f64>,
    #[serde(default)]
    extra_monthly_payment: Option<f64>,
    #[serde(default)]
    decimals: Option<f64>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            home_price: self.home_price,
            down_payment: self.down_payment,
            loan_years: self.loan_years,
            annual_interest_rate_percent: self.annual_interest_rate_percent,
            annual_property_tax: self.annual_property_tax,
            annual_insurance: self.annual_insurance,
            monthly_hoa: self.monthly_hoa,
            extra_monthly_payment: self.extra_monthly_payment,
            decimals: self.decimals,
        }
    }
}

/// Single source for the chat schema (and CLI). Every field is optional and
/// falls back to the documented default, so the tool always returns a result.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("home_price")
                .default(400_000.0)
                .min(0.0)
                .describe("Purchase price of the home in your currency. Default 400000."),
        )
        .param(
            Param::number("down_payment")
                .default(80_000.0)
                .min(0.0)
                .describe(
                    "Down-payment amount (a cash amount, not a percent). Must be at most \
                     home_price. Default 80000.",
                ),
        )
        .param(
            Param::number("loan_years")
                .default(30.0)
                .min(0.0)
                .max(100.0)
                .describe("Loan term in years, e.g. 30 or 15. Default 30 (max 100)."),
        )
        .param(
            Param::number("annual_interest_rate_percent")
                .default(6.5)
                .min(0.0)
                .max(100.0)
                .describe(
                    "Nominal annual interest rate as a percent, e.g. 6.5 for 6.5%. Default 6.5.",
                ),
        )
        .param(
            Param::number("annual_property_tax")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Annual property tax amount, spread evenly across the year. Default 0.",
                ),
        )
        .param(
            Param::number("annual_insurance")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Annual homeowner's insurance premium, spread evenly across the year. \
                     Default 0.",
                ),
        )
        .param(
            Param::number("monthly_hoa")
                .default(0.0)
                .min(0.0)
                .describe("Monthly HOA / condo association dues. Default 0."),
        )
        .param(
            Param::number("extra_monthly_payment")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Extra amount paid toward principal every month. Shortens the term and \
                     cuts total interest. Default 0.",
                ),
        )
        .param(
            Param::number("decimals")
                .default(2.0)
                .min(0.0)
                .max(10.0)
                .describe("Decimal places for money outputs. Default 2 (range 0–10)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mortgage-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Monthly mortgage payment (principal, interest, taxes, insurance, HOA) and total cost.",
    skill(
        description = "Compute the fixed-rate monthly mortgage payment and total cost of a home loan. Pass home_price and down_payment (a cash amount) — the financed loan_amount is their difference — plus loan_years (term) and annual_interest_rate_percent (nominal percent). Optionally add annual_property_tax, annual_insurance and monthly_hoa to roll taxes/insurance/HOA into the monthly payment, and extra_monthly_payment to pay down principal faster. Every parameter is optional with a sensible default. Returns loan_amount, monthly_principal_interest, monthly_taxes, monthly_insurance, monthly_hoa, monthly_payment, payoff_months, total_principal, total_interest, total_tax, total_insurance, total_hoa, total_cost and a plain-language summary. An extra monthly payment correctly shortens payoff_months and reduces total_interest.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mortgage-calculator", |a: Args| {
            gizza_ai_mortgage_calculator_core::compute_json(&a.inputs())
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
                    "home_price": { "type": "number", "minimum": 0, "default": 400000.0, "description": "Purchase price of the home in your currency. Default 400000." },
                    "down_payment": { "type": "number", "minimum": 0, "default": 80000.0, "description": "Down-payment amount (a cash amount, not a percent). Must be at most home_price. Default 80000." },
                    "loan_years": { "type": "number", "minimum": 0, "maximum": 100, "default": 30.0, "description": "Loan term in years, e.g. 30 or 15. Default 30 (max 100)." },
                    "annual_interest_rate_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 6.5, "description": "Nominal annual interest rate as a percent, e.g. 6.5 for 6.5%. Default 6.5." },
                    "annual_property_tax": { "type": "number", "minimum": 0, "default": 0.0, "description": "Annual property tax amount, spread evenly across the year. Default 0." },
                    "annual_insurance": { "type": "number", "minimum": 0, "default": 0.0, "description": "Annual homeowner's insurance premium, spread evenly across the year. Default 0." },
                    "monthly_hoa": { "type": "number", "minimum": 0, "default": 0.0, "description": "Monthly HOA / condo association dues. Default 0." },
                    "extra_monthly_payment": { "type": "number", "minimum": 0, "default": 0.0, "description": "Extra amount paid toward principal every month. Shortens the term and cuts total interest. Default 0." },
                    "decimals": { "type": "number", "minimum": 0, "maximum": 10, "default": 2.0, "description": "Decimal places for money outputs. Default 2 (range 0–10)." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
