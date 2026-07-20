//! gizza-ai/budget-planner — chat skill block on the shared tool abstraction.
//! Builds a 50/30/20 (or custom-split) budget, or a zero-based budget from
//! expense categories, and shows what's left to allocate. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure compute — runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    income: f64,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    split: String,
    #[serde(default)]
    expenses: String,
    #[serde(default)]
    currency: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("income")
                .required()
                .min(0.01)
                .max(1_000_000_000.0)
                .describe(
                    "Monthly take-home (after-tax) income — the amount that actually lands \
                     in your account each month, e.g. 4500. For an annual salary, divide \
                     by 12 first.",
                ),
        )
        .param(
            Param::enumv("mode", ["50-30-20", "zero-based"])
                .default("50-30-20")
                .describe(
                    "Budget method. `50-30-20` splits income into needs/wants/savings \
                     target buckets (adjust the shares with `split`); `zero-based` \
                     assigns income across your expense categories and shows what's \
                     left to allocate until every dollar has a job.",
                ),
        )
        .param(Param::string("split").default("50/30/20").describe(
            "Needs/wants/savings percentage shares for 50-30-20 mode — three numbers \
             summing to 100, e.g. `50/30/20`, `60/30/10`, or `80/0/20` for a two-bucket \
             80/20 plan. Separators can be `/`, commas or spaces. Ignored in zero-based \
             mode.",
        ))
        .param(Param::string("expenses").describe(
            "Expense categories, one per line (newlines or `;` separate entries): \
             `Name: amount`, e.g. `Rent: 1200`. Amounts may use `$` and thousands \
             separators. In 50-30-20 mode each line must end with a bucket tag \
             `(needs)`, `(wants)` or `(savings)` so planned spending is compared to \
             the targets; in zero-based mode tags are optional. Optional in 50-30-20 \
             mode, required for zero-based. Max 100 lines.",
        ))
        .param(
            Param::string("currency")
                .default("$")
                .describe("Currency symbol prefixed to amounts in the report (default `$`)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/budget-planner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a 50/30/20 or zero-based monthly budget and see what's left to allocate",
    skill(
        description = "Build a monthly budget from take-home (after-tax) income. In `50-30-20` mode it splits income into needs/wants/savings target buckets (customize the shares with `split`, e.g. 60/30/10) and, if you list bucket-tagged expenses, compares planned spending against each target. In `zero-based` mode it allocates income across your expense categories (`Name: amount`, one per line) and reports the exact amount left to allocate — surplus, deficit, or a perfectly zeroed budget. All arithmetic is exact to the cent; returns bucket targets, per-category shares of income, totals, and a one-line summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "budget-planner", |a: Args| {
            gizza_ai_budget_planner_core::plan(a.income, &a.mode, &a.split, &a.expenses, &a.currency)
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
                    "income": { "type": "number", "minimum": 0.01, "maximum": 1000000000, "description": "Monthly take-home (after-tax) income — the amount that actually lands in your account each month, e.g. 4500. For an annual salary, divide by 12 first." },
                    "mode": { "type": "string", "enum": ["50-30-20", "zero-based"], "default": "50-30-20", "description": "Budget method. `50-30-20` splits income into needs/wants/savings target buckets (adjust the shares with `split`); `zero-based` assigns income across your expense categories and shows what's left to allocate until every dollar has a job." },
                    "split": { "type": "string", "default": "50/30/20", "description": "Needs/wants/savings percentage shares for 50-30-20 mode — three numbers summing to 100, e.g. `50/30/20`, `60/30/10`, or `80/0/20` for a two-bucket 80/20 plan. Separators can be `/`, commas or spaces. Ignored in zero-based mode." },
                    "expenses": { "type": "string", "description": "Expense categories, one per line (newlines or `;` separate entries): `Name: amount`, e.g. `Rent: 1200`. Amounts may use `$` and thousands separators. In 50-30-20 mode each line must end with a bucket tag `(needs)`, `(wants)` or `(savings)` so planned spending is compared to the targets; in zero-based mode tags are optional. Optional in 50-30-20 mode, required for zero-based. Max 100 lines." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol prefixed to amounts in the report (default `$`)." }
                },
                "required": ["income"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
