//! gizza-ai/subscription-finder — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI
//! and page); handle() delegates to block_utils::run_skill. Pure compute — no host
//! calls; the statement text never leaves the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    transactions: String,
    /// How many times a merchant+amount must repeat to flag as recurring; the
    /// core clamps to 2..=24 (0/1 → 2).
    #[serde(default)]
    min_occurrences: u32,
    #[serde(default)]
    currency: String,
    #[serde(default)]
    date_format: String,
}

/// Single source for the chat schema (and CLI + page). Bounds on `min_occurrences`
/// reference the core clamp so the LLM-facing schema can't drift from what `find`
/// actually enforces.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("transactions")
                .required()
                .describe("The statement rows, one per line as \"date, description, amount\" (e.g. 2026-01-15, Netflix, 15.99). Commas inside a description are fine — only the first field is the date and the last is the amount. Blank and unparseable lines (like a header row) are ignored. Amounts may include a currency symbol, grouping commas, or a minus/parentheses for a debit."),
        )
        .param(
            Param::integer("min_occurrences")
                .default(2)
                .min(gizza_ai_subscription_finder_core::MIN_OCCURRENCES as f64)
                .max(gizza_ai_subscription_finder_core::MAX_OCCURRENCES as f64)
                .describe("How many times a merchant+amount must repeat before it counts as recurring, 2-24. Default 2. Raise it to only surface well-established subscriptions."),
        )
        .param(
            Param::string("currency")
                .default("$")
                .describe("Currency symbol to prefix amounts with in the report (e.g. $, £, €). Default '$'. Purely cosmetic — the tool does not convert currencies."),
        )
        .param(
            Param::enumv("date_format", ["auto", "iso", "us", "eu"])
                .default("auto")
                .describe("How to read the date column. 'auto' (default) detects ISO (YYYY-MM-DD) by the dash and disambiguates slash dates by any day > 12; 'iso' = YYYY-MM-DD; 'us' = MM/DD/YYYY; 'eu' = DD/MM/YYYY."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/subscription-finder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find recurring charges and subscriptions in pasted bank/card transactions.",
    skill(
        description = "Find recurring charges and subscriptions in a pasted list of bank or card transactions — privately, with no bank linking. Paste the statement as one 'date, description, amount' row per line (transactions param). The tool groups repeat charges from the same merchant (merging near-equal amounts within tolerance), detects each one's cadence (weekly / biweekly / monthly / quarterly / semiannual / annual), estimates the next charge date, and projects the recurring monthly and annual spend, ranked by yearly cost. Use min_occurrences (default 2, 2-24) to set how many repeats flag a charge as recurring, currency for the display symbol, and date_format (auto/iso/us/eu) for the date column.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "subscription-finder", |a: Args| {
            gizza_ai_subscription_finder_core::find(
                &a.transactions,
                a.min_occurrences,
                &a.currency,
                &a.date_format,
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
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "transactions": { "type": "string", "description": "The statement rows, one per line as \"date, description, amount\" (e.g. 2026-01-15, Netflix, 15.99). Commas inside a description are fine — only the first field is the date and the last is the amount. Blank and unparseable lines (like a header row) are ignored. Amounts may include a currency symbol, grouping commas, or a minus/parentheses for a debit." },
                    "min_occurrences": { "type": "integer", "minimum": 2, "maximum": 24, "default": 2, "description": "How many times a merchant+amount must repeat before it counts as recurring, 2-24. Default 2. Raise it to only surface well-established subscriptions." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol to prefix amounts with in the report (e.g. $, £, €). Default '$'. Purely cosmetic — the tool does not convert currencies." },
                    "date_format": { "type": "string", "enum": ["auto", "iso", "us", "eu"], "default": "auto", "description": "How to read the date column. 'auto' (default) detects ISO (YYYY-MM-DD) by the dash and disambiguates slash dates by any day > 12; 'iso' = YYYY-MM-DD; 'us' = MM/DD/YYYY; 'eu' = DD/MM/YYYY." }
                },
                "required": ["transactions"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
