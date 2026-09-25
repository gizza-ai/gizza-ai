//! gizza-ai/financial-ratio-analyzer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI and page manifest); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    figures: String,
    #[serde(default)]
    prior_figures: String,
    #[serde(default = "default_groups")]
    groups: String,
    #[serde(default = "default_basis")]
    basis: String,
    #[serde(default = "default_days_in_period")]
    days_in_period: i64,
    #[serde(default = "default_benchmarks")]
    benchmarks: bool,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_groups() -> String {
    "all".into()
}
fn default_basis() -> String {
    "average".into()
}
fn default_days_in_period() -> i64 {
    365
}
fn default_benchmarks() -> bool {
    true
}
fn default_decimals() -> i64 {
    2
}
fn default_currency() -> String {
    "$".into()
}
fn default_output() -> String {
    "summary".into()
}

const FIGURES_DESC: &str = "Current-period income-statement and balance-sheet figures as pasted `label: value` lines. Labels can be common statement names such as Revenue, COGS, Current assets, Current liabilities, Total assets, Total liabilities, Total equity, Net income, Cash, Inventory, Accounts receivable, Accounts payable, Long term debt, Shares outstanding, or Share price. Values may include currency symbols, commas, accounting parentheses, and k/m/bn suffixes. Up to 400 non-blank lines.";
const PRIOR_FIGURES_DESC: &str = "Optional prior-period statement in the same `label: value` format. When supplied, the report adds prior and change columns; with basis=average, balance-sheet denominators such as assets, equity, inventory, receivables and payables use the average of current and prior balances.";
const GROUPS_DESC: &str = "Ratio family to show. all (default) reports liquidity, leverage and solvency, margins, returns, efficiency, and market ratios when share data is available. Choose liquidity, leverage, margins, returns, efficiency, or market to narrow the output.";
const BASIS_DESC: &str = "Balance-sheet denominator basis for turnover and return ratios. average (default) uses average current/prior balances when prior_figures is provided and falls back to ending balances otherwise. ending always uses the current-period balances.";
const DAYS_DESC: &str = "Days in the period used for DSO, DIO, DPO and cash conversion cycle calculations. Default 365; use 360 for banker-style years, 90 for a quarter, or 30 for a month. Must be 1 to 366.";
const BENCHMARKS_DESC: &str = "Show generic rule-of-thumb benchmark statuses and a simple health score. Default true. These are educational ranges only, not industry-specific financial advice.";
const DECIMALS_DESC: &str = "Decimal places for readable summary and table output, from 0 to 6. Default 2. CSV and JSON outputs retain machine-readable numeric precision.";
const CURRENCY_DESC: &str = "Currency symbol to prefix money values in readable output, such as $, €, £, ¥, R$ or CHF plus a space. Default $. Cosmetic only; no currency conversion is performed.";
const OUTPUT_DESC: &str = "Output shape: summary (default) for a full readable report, table for grouped aligned rows, csv for spreadsheets, or json for a machine-readable analysis object.";

/// Single source for the chat schema, CLI, and synced page manifest.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("figures").required().describe(FIGURES_DESC))
        .param(
            Param::string("prior_figures")
                .default("")
                .describe(PRIOR_FIGURES_DESC),
        )
        .param(
            Param::enumv(
                "groups",
                [
                    "all",
                    "liquidity",
                    "leverage",
                    "margins",
                    "returns",
                    "efficiency",
                    "market",
                ],
            )
            .default("all")
            .describe(GROUPS_DESC),
        )
        .param(
            Param::enumv("basis", ["average", "ending"])
                .default("average")
                .describe(BASIS_DESC),
        )
        .param(
            Param::integer("days_in_period")
                .min(1.0)
                .max(366.0)
                .default(365)
                .describe(DAYS_DESC),
        )
        .param(
            Param::boolean("benchmarks")
                .default(true)
                .describe(BENCHMARKS_DESC),
        )
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(6.0)
                .default(2)
                .describe(DECIMALS_DESC),
        )
        .param(
            Param::string("currency")
                .default("$")
                .describe(CURRENCY_DESC),
        )
        .param(
            Param::enumv("output", ["summary", "table", "csv", "json"])
                .default("summary")
                .describe(OUTPUT_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/financial-ratio-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute liquidity, leverage, margin, return, efficiency and market ratios from pasted financial statements",
    skill(
        description = "Analyze pasted income-statement and balance-sheet figures and compute standard financial ratios across liquidity, leverage and solvency, margins, returns, efficiency, and market valuation. Paste `label: value` lines using common statement labels; values may include currency symbols, thousands separators, accounting parentheses for negatives, and k/m/bn suffixes. The tool derives omitted subtotals where possible, reports missing inputs as n/a with the exact requirement, checks balance-sheet consistency, can compare a prior period, and can use average balance-sheet denominators for return and turnover ratios. Outputs include readable summaries, grouped tables, CSV, and JSON. Generic benchmark statuses and a health score are available for educational use only; this is not financial, investment, tax, or accounting advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "financial-ratio-analyzer", |a: Args| {
            gizza_ai_financial_ratio_analyzer_core::run(
                &a.figures,
                &a.prior_figures,
                &a.groups,
                &a.basis,
                a.days_in_period,
                a.benchmarks,
                a.decimals,
                &a.currency,
                &a.output,
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

    #[test]
    fn descriptor_documents_every_param() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 9);
        for (name, spec) in props {
            assert!(
                spec["description"].as_str().unwrap_or_default().len() > 40,
                "{name} needs a useful description"
            );
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["figures"],
            "properties": {
                "figures": { "type": "string", "description": FIGURES_DESC },
                "prior_figures": { "type": "string", "default": "", "description": PRIOR_FIGURES_DESC },
                "groups": { "type": "string", "enum": ["all", "liquidity", "leverage", "margins", "returns", "efficiency", "market"], "default": "all", "description": GROUPS_DESC },
                "basis": { "type": "string", "enum": ["average", "ending"], "default": "average", "description": BASIS_DESC },
                "days_in_period": { "type": "integer", "minimum": 1, "maximum": 366, "default": 365, "description": DAYS_DESC },
                "benchmarks": { "type": "boolean", "default": true, "description": BENCHMARKS_DESC },
                "decimals": { "type": "integer", "minimum": 0, "maximum": 6, "default": 2, "description": DECIMALS_DESC },
                "currency": { "type": "string", "default": "$", "description": CURRENCY_DESC },
                "output": { "type": "string", "enum": ["summary", "table", "csv", "json"], "default": "summary", "description": OUTPUT_DESC }
            }
        });
        assert_eq!(derived, authored);
    }
}
