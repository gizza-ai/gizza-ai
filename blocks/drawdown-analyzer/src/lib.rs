//! gizza-ai/drawdown-analyzer — drawdown analytics for an equity curve or a
//! periodic-returns series. Thin wrapper; the chat schema is single-sourced from
//! descriptor() (which also drives the CLI) and handle() delegates to
//! run_skill → core::analyze (the structured form). Pure → all backends.
//! Educational only, not financial advice.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_drawdown_analyzer_core::analyze;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    series: String,
    #[serde(default = "default_series_type")]
    series_type: String,
    #[serde(default = "default_frequency")]
    frequency: String,
    #[serde(default)]
    start_date: String,
    #[serde(default)]
    has_header: bool,
    #[serde(default = "default_top_n")]
    top_n: f64,
    #[serde(default)]
    recovery_cagr: f64,
}

fn default_series_type() -> String {
    "equity".to_string()
}

fn default_frequency() -> String {
    "period".to_string()
}

fn default_top_n() -> f64 {
    5.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("series").required().describe(
                "The series to analyze, one observation per line or separated by commas/spaces: equity/balance levels (10000, 10420, 9880) or periodic returns when series_type is returns. Returns may be decimals (0.012) or percents (1.2%). Rows may instead be date,value pairs (2020-01-31,10000) with YYYY-MM-DD dates in oldest-first order, which date the peaks, troughs and recoveries. Needs 2 to 20000 observations.",
            ),
        )
        .param(
            Param::enumv("series_type", ["equity", "returns"])
                .default("equity")
                .describe(
                    "How to read the values: equity for account balances or price/index levels (must be greater than 0), returns for periodic returns that are compounded into a curve first. Default equity.",
                ),
        )
        .param(
            Param::enumv(
                "frequency",
                ["period", "daily", "trading", "weekly", "monthly", "quarterly", "annual"],
            )
            .default("period")
            .describe(
                "How far apart the observations are. Sets the duration unit in the report and, with start_date, places observations on the calendar: period (unitless, the default), daily (every calendar day), trading (weekdays only, no holiday calendar), weekly, monthly, quarterly, annual.",
            ),
        )
        .param(
            Param::string("start_date")
                .default("")
                .describe(
                    "Calendar date of the FIRST observation as YYYY-MM-DD (e.g. 2020-01-31), used with frequency to date every later observation. Leave empty for positions only. Ignored when the pasted rows already carry a date column, and rejected when frequency is period.",
                ),
        )
        .param(
            Param::boolean("has_header").default(false).describe(
                "Skip the first line before parsing when the pasted series starts with a column label such as balance or date,value. Default false.",
            ),
        )
        .param(
            Param::integer("top_n")
                .default(5)
                .min(1.0)
                .max(20.0)
                .describe(
                    "How many of the deepest drawdown episodes to list, deepest first, from 1 to 20. The total episode count is reported in full regardless. Default 5.",
                ),
        )
        .param(
            Param::number("recovery_cagr").default(0.0).min(0.0).max(50.0).describe(
                "Assumed annual growth rate as a percent (e.g. 8 means 8% a year) used to estimate how many years it would take to earn back the deepest drawdown. 0 turns the estimate off. Default 0.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DrawdownAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/drawdown-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Max drawdown, every drawdown episode, recovery time and ulcer index from an equity or returns series",
    skill(
        description = "Analyze the drawdowns of an equity curve or a periodic-returns series: maximum drawdown, the gain needed to erase it, the current drawdown, every drawdown episode ranked by depth with its peak, trough, decline length, recovery length and total underwater stretch, plus average drawdown, longest underwater stretch, share of time underwater, ulcer index, pain index and the underwater curve. Drawdown is measured against the series' own running peak; an episode ends only when the series closes back at or above that peak, and one still underwater at the last observation is reported as ongoing. Paste equity/balance levels or returns (decimals or percents), optionally as date,value rows or with a start_date plus a frequency to date the peaks and troughs. Set recovery_cagr to estimate the years needed to recover at an assumed annual growth rate. Runs locally. Educational only, not financial advice.",
        parameters = schema_json()
    ),
)]
impl DrawdownAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "drawdown-analyzer", |a: Args| {
            if !a.top_n.is_finite() || a.top_n.fract() != 0.0 {
                return Err(SkillError::InvalidArgs(
                    "top_n must be a whole number between 1 and 20".into(),
                ));
            }
            let top_n = a.top_n as i64;
            if !(1..=20).contains(&top_n) {
                return Err(SkillError::InvalidArgs(format!(
                    "top_n must be between 1 and 20, got {top_n}"
                )));
            }
            analyze(
                &a.series,
                &a.series_type,
                &a.frequency,
                &a.start_date,
                a.has_header,
                top_n as usize,
                a.recovery_cagr,
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "series": { "type": "string", "description": "The series to analyze, one observation per line or separated by commas/spaces: equity/balance levels (10000, 10420, 9880) or periodic returns when series_type is returns. Returns may be decimals (0.012) or percents (1.2%). Rows may instead be date,value pairs (2020-01-31,10000) with YYYY-MM-DD dates in oldest-first order, which date the peaks, troughs and recoveries. Needs 2 to 20000 observations." },
                    "series_type": { "type": "string", "enum": ["equity", "returns"], "default": "equity", "description": "How to read the values: equity for account balances or price/index levels (must be greater than 0), returns for periodic returns that are compounded into a curve first. Default equity." },
                    "frequency": { "type": "string", "enum": ["period", "daily", "trading", "weekly", "monthly", "quarterly", "annual"], "default": "period", "description": "How far apart the observations are. Sets the duration unit in the report and, with start_date, places observations on the calendar: period (unitless, the default), daily (every calendar day), trading (weekdays only, no holiday calendar), weekly, monthly, quarterly, annual." },
                    "start_date": { "type": "string", "default": "", "description": "Calendar date of the FIRST observation as YYYY-MM-DD (e.g. 2020-01-31), used with frequency to date every later observation. Leave empty for positions only. Ignored when the pasted rows already carry a date column, and rejected when frequency is period." },
                    "has_header": { "type": "boolean", "default": false, "description": "Skip the first line before parsing when the pasted series starts with a column label such as balance or date,value. Default false." },
                    "top_n": { "type": "integer", "minimum": 1, "maximum": 20, "default": 5, "description": "How many of the deepest drawdown episodes to list, deepest first, from 1 to 20. The total episode count is reported in full regardless. Default 5." },
                    "recovery_cagr": { "type": "number", "minimum": 0, "maximum": 50, "default": 0.0, "description": "Assumed annual growth rate as a percent (e.g. 8 means 8% a year) used to estimate how many years it would take to earn back the deepest drawdown. 0 turns the estimate off. Default 0." }
                },
                "required": ["series"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn every_param_is_documented() {
        for p in descriptor().params {
            assert!(
                !p.description.is_empty(),
                "param {} needs a describe()",
                p.name
            );
        }
    }
}
