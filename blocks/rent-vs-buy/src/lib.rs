//! gizza-ai/rent-vs-buy — chat skill block on the shared tool abstraction.
//!
//! Compares the long-run financial outcome of renting versus buying a home using the
//! standard "invest the difference" net-worth race over a chosen horizon. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill` and returns the pretty result JSON from
//! `core::compute_json`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rent_vs_buy_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    home_price: Option<f64>,
    #[serde(default)]
    down_payment_percent: Option<f64>,
    #[serde(default)]
    mortgage_rate_percent: Option<f64>,
    #[serde(default)]
    loan_term_years: Option<f64>,
    #[serde(default)]
    monthly_rent: Option<f64>,
    #[serde(default)]
    years: Option<f64>,
    #[serde(default)]
    home_appreciation_percent: Option<f64>,
    #[serde(default)]
    rent_growth_percent: Option<f64>,
    #[serde(default)]
    investment_return_percent: Option<f64>,
    #[serde(default)]
    property_tax_percent: Option<f64>,
    #[serde(default)]
    home_insurance_percent: Option<f64>,
    #[serde(default)]
    maintenance_percent: Option<f64>,
    #[serde(default)]
    hoa_monthly: Option<f64>,
    #[serde(default)]
    buying_closing_percent: Option<f64>,
    #[serde(default)]
    selling_cost_percent: Option<f64>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    decimals: Option<f64>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            home_price: self.home_price,
            down_payment_percent: self.down_payment_percent,
            mortgage_rate_percent: self.mortgage_rate_percent,
            loan_term_years: self.loan_term_years,
            monthly_rent: self.monthly_rent,
            years: self.years,
            home_appreciation_percent: self.home_appreciation_percent,
            rent_growth_percent: self.rent_growth_percent,
            investment_return_percent: self.investment_return_percent,
            property_tax_percent: self.property_tax_percent,
            home_insurance_percent: self.home_insurance_percent,
            maintenance_percent: self.maintenance_percent,
            hoa_monthly: self.hoa_monthly,
            buying_closing_percent: self.buying_closing_percent,
            selling_cost_percent: self.selling_cost_percent,
            currency: self.currency,
            decimals: self.decimals,
        }
    }
}

/// Single source for the chat schema (and CLI). Every field is optional and falls back
/// to the documented default, so the tool always returns a result.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("home_price")
                .default(400_000.0)
                .min(0.0)
                .describe("Home purchase price, in your currency. Default 400000."),
        )
        .param(
            Param::number("down_payment_percent")
                .default(20.0)
                .min(0.0)
                .max(100.0)
                .describe("Down payment as a percent of the price. Default 20."),
        )
        .param(
            Param::number("mortgage_rate_percent")
                .default(6.5)
                .min(0.0)
                .describe("Nominal annual mortgage interest rate (percent). Default 6.5."),
        )
        .param(
            Param::number("loan_term_years")
                .default(30.0)
                .min(1.0)
                .max(100.0)
                .describe("Mortgage term in years. Default 30."),
        )
        .param(
            Param::number("monthly_rent")
                .default(2_000.0)
                .min(0.0)
                .describe("Current monthly rent for a comparable place. Default 2000."),
        )
        .param(
            Param::number("years")
                .default(10.0)
                .min(1.0)
                .max(100.0)
                .describe(
                    "How many years you plan to stay — the comparison horizon. This is the \
                     single biggest driver of the result. Default 10.",
                ),
        )
        .param(
            Param::number("home_appreciation_percent")
                .default(3.0)
                .describe("Annual home-value appreciation (percent). Default 3."),
        )
        .param(
            Param::number("rent_growth_percent")
                .default(3.0)
                .describe("Annual rent increase (percent). Default 3."),
        )
        .param(
            Param::number("investment_return_percent")
                .default(5.0)
                .describe(
                    "Annual after-tax return the renter earns on the invested down payment and \
                     monthly savings (percent). Higher returns favour renting. Default 5.",
                ),
        )
        .param(
            Param::number("property_tax_percent")
                .default(1.1)
                .describe("Annual property tax as a percent of home value. Default 1.1."),
        )
        .param(
            Param::number("home_insurance_percent")
                .default(0.5)
                .describe("Annual home insurance as a percent of home value. Default 0.5."),
        )
        .param(
            Param::number("maintenance_percent")
                .default(1.0)
                .describe("Annual maintenance/repairs as a percent of home value. Default 1."),
        )
        .param(
            Param::number("hoa_monthly")
                .default(0.0)
                .min(0.0)
                .describe("Monthly HOA / condo dues, in your currency. Default 0."),
        )
        .param(
            Param::number("buying_closing_percent")
                .default(3.0)
                .describe(
                    "Buying closing costs as a percent of the price, paid up front. Default 3.",
                ),
        )
        .param(
            Param::number("selling_cost_percent")
                .default(6.0)
                .describe(
                    "Selling costs (agent commission + closing) as a percent of the eventual \
                     sale price. Default 6.",
                ),
        )
        .param(
            Param::string("currency")
                .default("$")
                .describe("Currency symbol prefixed to amounts in the summary. Default $."),
        )
        .param(
            Param::number("decimals")
                .default(0.0)
                .min(0.0)
                .max(10.0)
                .describe("Decimal places for money outputs. Default 0 (range 0–10)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rent-vs-buy",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare the long-run financial outcome of renting versus buying a home, with a break-even year and net-worth verdict.",
    skill(
        description = "Compare the long-run financial outcome of renting versus buying a home using the standard invest-the-difference net-worth race, not a naive monthly rent-vs-mortgage comparison. Pass home_price, down_payment_percent, mortgage_rate_percent and loan_term_years for the purchase; monthly_rent for the alternative; and years for how long you'll stay (the biggest driver). Tune home_appreciation_percent, rent_growth_percent, investment_return_percent (higher returns favour renting), property_tax_percent, home_insurance_percent, maintenance_percent, hoa_monthly, buying_closing_percent and selling_cost_percent. The buyer's up-front cash is invested by the renter as opportunity cost; each month whoever pays less invests the difference; both funds compound. Every parameter is optional with a sensible default. Returns loan_amount, down_payment, total_upfront_cost, monthly_principal_interest, first_month_buy_cost, first_month_rent_cost, buy_net_worth, rent_net_worth, net_worth_difference, verdict (buy|rent|even), break_even_year, home_value_at_horizon, remaining_mortgage_at_horizon, total_rent_paid, a year-by-year net-worth race, and a plain-language summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rent-vs-buy", |a: Args| {
            gizza_ai_rent_vs_buy_core::compute_json(&a.inputs()).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored schema,
    /// so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "home_price": { "type": "number", "minimum": 0, "default": 400000.0, "description": "Home purchase price, in your currency. Default 400000." },
                    "down_payment_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 20.0, "description": "Down payment as a percent of the price. Default 20." },
                    "mortgage_rate_percent": { "type": "number", "minimum": 0, "default": 6.5, "description": "Nominal annual mortgage interest rate (percent). Default 6.5." },
                    "loan_term_years": { "type": "number", "minimum": 1, "maximum": 100, "default": 30.0, "description": "Mortgage term in years. Default 30." },
                    "monthly_rent": { "type": "number", "minimum": 0, "default": 2000.0, "description": "Current monthly rent for a comparable place. Default 2000." },
                    "years": { "type": "number", "minimum": 1, "maximum": 100, "default": 10.0, "description": "How many years you plan to stay — the comparison horizon. This is the single biggest driver of the result. Default 10." },
                    "home_appreciation_percent": { "type": "number", "default": 3.0, "description": "Annual home-value appreciation (percent). Default 3." },
                    "rent_growth_percent": { "type": "number", "default": 3.0, "description": "Annual rent increase (percent). Default 3." },
                    "investment_return_percent": { "type": "number", "default": 5.0, "description": "Annual after-tax return the renter earns on the invested down payment and monthly savings (percent). Higher returns favour renting. Default 5." },
                    "property_tax_percent": { "type": "number", "default": 1.1, "description": "Annual property tax as a percent of home value. Default 1.1." },
                    "home_insurance_percent": { "type": "number", "default": 0.5, "description": "Annual home insurance as a percent of home value. Default 0.5." },
                    "maintenance_percent": { "type": "number", "default": 1.0, "description": "Annual maintenance/repairs as a percent of home value. Default 1." },
                    "hoa_monthly": { "type": "number", "minimum": 0, "default": 0.0, "description": "Monthly HOA / condo dues, in your currency. Default 0." },
                    "buying_closing_percent": { "type": "number", "default": 3.0, "description": "Buying closing costs as a percent of the price, paid up front. Default 3." },
                    "selling_cost_percent": { "type": "number", "default": 6.0, "description": "Selling costs (agent commission + closing) as a percent of the eventual sale price. Default 6." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol prefixed to amounts in the summary. Default $." },
                    "decimals": { "type": "number", "minimum": 0, "maximum": 10, "default": 0.0, "description": "Decimal places for money outputs. Default 0 (range 0–10)." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
