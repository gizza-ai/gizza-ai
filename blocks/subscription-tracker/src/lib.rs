//! gizza-ai/subscription-tracker — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI
//! and page); handle() delegates to block_utils::run_skill. Pure compute — no host
//! calls; the subscription list never leaves the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    subscriptions: String,
    #[serde(default)]
    default_cycle: String,
    #[serde(default)]
    currency: String,
    #[serde(default)]
    sort: String,
}

/// Single source for the chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("subscriptions")
                .required()
                .describe("Your subscriptions, one per line as `Name: amount [cycle]` (e.g. `Netflix: 15.99 monthly`). The `:` or `=` is optional — `Netflix 15.99 monthly` works too. The cycle keyword is optional and falls back to `default_cycle`; accepted cycles are daily, weekly, biweekly, monthly, quarterly, semiannual and yearly (plus synonyms like mo, /yr, wk, annually). Amounts may include a currency symbol and grouping commas. Blank lines and lines starting with `#` are ignored. Max 200 lines."),
        )
        .param(
            Param::enumv(
                "default_cycle",
                ["weekly", "biweekly", "monthly", "quarterly", "semiannual", "yearly"],
            )
            .default("monthly")
            .describe("Billing cycle assumed for any line that omits one. Default `monthly`. Annualization multipliers: weekly ×52, biweekly ×26, monthly ×12, quarterly ×4, semiannual ×2, yearly ×1."),
        )
        .param(
            Param::string("currency")
                .default("$")
                .describe("Currency symbol prefixed to every amount in the report (e.g. $, £, €). Default `$`. Cosmetic only — the tool does not convert between currencies."),
        )
        .param(
            Param::enumv("sort", ["cost", "name", "input"])
                .default("cost")
                .describe("Row order. `cost` (default) lists the biggest annual spend first, `name` sorts alphabetically, `input` keeps your paste order. The cancel-candidate callout is always the biggest annual spend regardless of sort."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/subscription-tracker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Total your subscriptions, annualize every billing cycle, and flag the biggest spend to cancel.",
    skill(
        description = "Add up your recurring subscriptions and see what they really cost — privately, in your browser, with no accounts or bank linking. Paste your plans as one `Name: amount [cycle]` per line (subscriptions param). The tool normalizes every plan to a common monthly and annual figure (weekly ×52, biweekly ×26, monthly ×12, quarterly ×4, semiannual ×2, yearly ×1), reports the combined monthly / annual / 5-year / per-day spend, shows each plan's share of the total, and flags the single biggest annual spend as a cancel candidate with the yearly savings. Use default_cycle for lines that omit a cycle, currency for the display symbol, and sort (cost/name/input) for the row order.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "subscription-tracker", |a: Args| {
            gizza_ai_subscription_tracker_core::track(
                &a.subscriptions,
                &a.default_cycle,
                &a.currency,
                &a.sort,
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
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "subscriptions": { "type": "string", "description": "Your subscriptions, one per line as `Name: amount [cycle]` (e.g. `Netflix: 15.99 monthly`). The `:` or `=` is optional — `Netflix 15.99 monthly` works too. The cycle keyword is optional and falls back to `default_cycle`; accepted cycles are daily, weekly, biweekly, monthly, quarterly, semiannual and yearly (plus synonyms like mo, /yr, wk, annually). Amounts may include a currency symbol and grouping commas. Blank lines and lines starting with `#` are ignored. Max 200 lines." },
                    "default_cycle": { "type": "string", "enum": ["weekly", "biweekly", "monthly", "quarterly", "semiannual", "yearly"], "default": "monthly", "description": "Billing cycle assumed for any line that omits one. Default `monthly`. Annualization multipliers: weekly ×52, biweekly ×26, monthly ×12, quarterly ×4, semiannual ×2, yearly ×1." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol prefixed to every amount in the report (e.g. $, £, €). Default `$`. Cosmetic only — the tool does not convert between currencies." },
                    "sort": { "type": "string", "enum": ["cost", "name", "input"], "default": "cost", "description": "Row order. `cost` (default) lists the biggest annual spend first, `name` sorts alphabetically, `input` keeps your paste order. The cancel-candidate callout is always the biggest annual spend regardless of sort." }
                },
                "required": ["subscriptions"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
