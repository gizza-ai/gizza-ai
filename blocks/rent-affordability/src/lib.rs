//! gizza-ai/rent-affordability — chat skill block on the shared tool abstraction.
//!
//! Answers "how much rent can I afford?" from an income figure using the classic
//! 30% rule (or any custom rent-to-income ratio), optionally factoring existing
//! monthly debts via a back-end DTI cap and converting a net (take-home) income to
//! gross with a flat assumed tax rate. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill` and returns the pretty result JSON from
//! `core::compute_json`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rent_affordability_core::Inputs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    income: Option<f64>,
    #[serde(default)]
    income_period: Option<String>,
    #[serde(default)]
    income_type: Option<String>,
    #[serde(default)]
    tax_rate_percent: Option<f64>,
    #[serde(default)]
    rent_to_income_ratio: Option<f64>,
    #[serde(default)]
    monthly_debts: Option<f64>,
    #[serde(default)]
    max_dti_ratio: Option<f64>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    decimals: Option<f64>,
}

impl Args {
    fn inputs(self) -> Inputs {
        Inputs {
            income: self.income,
            income_period: self.income_period,
            income_type: self.income_type,
            tax_rate_percent: self.tax_rate_percent,
            rent_to_income_ratio: self.rent_to_income_ratio,
            monthly_debts: self.monthly_debts,
            max_dti_ratio: self.max_dti_ratio,
            currency: self.currency,
            decimals: self.decimals,
        }
    }
}

/// Single source for the chat schema (and CLI). Every field is optional and
/// falls back to the documented default, so the tool always returns a result.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("income")
                .default(60_000.0)
                .min(0.0)
                .describe("Your income amount, in your currency. Default 60000."),
        )
        .param(
            Param::enumv("income_period", ["annual", "monthly"])
                .default("annual")
                .describe("Whether `income` is an annual (default) or monthly figure."),
        )
        .param(
            Param::enumv("income_type", ["gross", "net"])
                .default("gross")
                .describe(
                    "Whether `income` is gross/pre-tax (default) or net/take-home. A net income \
                     is grossed up using tax_rate_percent before the ratio is applied.",
                ),
        )
        .param(
            Param::number("tax_rate_percent")
                .default(25.0)
                .min(0.0)
                .max(100.0)
                .describe(
                    "Assumed tax + deductions rate (percent) used to gross up a net income. \
                     Only used when income_type=net. Default 25.",
                ),
        )
        .param(
            Param::number("rent_to_income_ratio")
                .default(30.0)
                .min(1.0)
                .max(100.0)
                .describe(
                    "Rent-to-income ratio as a percent — the rule. 30 is the classic 30% rule; \
                     10–50 is the common range. Default 30.",
                ),
        )
        .param(
            Param::number("monthly_debts")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Existing recurring monthly debt payments (car, student loans, credit cards). \
                     Default 0.",
                ),
        )
        .param(
            Param::number("max_dti_ratio")
                .default(36.0)
                .min(1.0)
                .max(100.0)
                .describe(
                    "Back-end debt-to-income cap (percent): rent + debts must stay under this \
                     share of gross monthly income. Default 36.",
                ),
        )
        .param(
            Param::string("currency")
                .default("$")
                .describe("Currency symbol prefixed to amounts in the summary. Default $."),
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
    name = "gizza-ai/rent-affordability",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "How much rent you can afford from your income using the 30% (or custom) rule, adjusted for debts.",
    skill(
        description = "Work out the maximum affordable monthly rent from an income figure using the classic 30% rule or any custom rent-to-income ratio. Pass income plus income_period (annual|monthly) and income_type (gross|net) — a net income is grossed up with tax_rate_percent. Set rent_to_income_ratio for the rule (default 30), and add monthly_debts with max_dti_ratio (default 36% back-end DTI) to pull the recommendation down when existing debts apply. Every parameter is optional with a sensible default. Returns gross_annual_income, gross_monthly_income, max_affordable_rent, debt_adjusted_max_rent, recommended_max_rent (the smaller of the two), annual_rent_at_recommended, remaining_after_rent_and_debts, income_to_rent_multiple, a 25%/30%/35% guideline_range and a plain-language summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rent-affordability", |a: Args| {
            gizza_ai_rent_affordability_core::compute_json(&a.inputs())
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
                    "income": { "type": "number", "minimum": 0, "default": 60000.0, "description": "Your income amount, in your currency. Default 60000." },
                    "income_period": { "type": "string", "enum": ["annual", "monthly"], "default": "annual", "description": "Whether `income` is an annual (default) or monthly figure." },
                    "income_type": { "type": "string", "enum": ["gross", "net"], "default": "gross", "description": "Whether `income` is gross/pre-tax (default) or net/take-home. A net income is grossed up using tax_rate_percent before the ratio is applied." },
                    "tax_rate_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 25.0, "description": "Assumed tax + deductions rate (percent) used to gross up a net income. Only used when income_type=net. Default 25." },
                    "rent_to_income_ratio": { "type": "number", "minimum": 1, "maximum": 100, "default": 30.0, "description": "Rent-to-income ratio as a percent — the rule. 30 is the classic 30% rule; 10–50 is the common range. Default 30." },
                    "monthly_debts": { "type": "number", "minimum": 0, "default": 0.0, "description": "Existing recurring monthly debt payments (car, student loans, credit cards). Default 0." },
                    "max_dti_ratio": { "type": "number", "minimum": 1, "maximum": 100, "default": 36.0, "description": "Back-end debt-to-income cap (percent): rent + debts must stay under this share of gross monthly income. Default 36." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol prefixed to amounts in the summary. Default $." },
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
