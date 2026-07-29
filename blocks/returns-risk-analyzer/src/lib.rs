//! gizza-ai/returns-risk-analyzer — performance & risk metrics for a returns
//! series. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends. Educational only, not advice.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_returns_risk_analyzer_core::analyze;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    returns: String,
    #[serde(default = "default_ppy")]
    periods_per_year: String,
    #[serde(default)]
    risk_free_rate: f64,
    #[serde(default)]
    target_return: f64,
    #[serde(default)]
    has_header: bool,
}

fn default_ppy() -> String {
    "252".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("returns").required().describe(
                "The periodic return series, one return per line or separated by commas/spaces. Each value is a decimal (0.012) or a percent with a % sign (1.2%). Needs at least 2 returns.",
            ),
        )
        .param(
            Param::enumv("periods_per_year", ["252", "52", "26", "12", "4", "1"])
                .default("252")
                .describe(
                    "How many return periods make up a year, used to annualize: 252 daily, 52 weekly, 26 biweekly, 12 monthly, 4 quarterly, 1 annual. Default 252 (daily).",
                ),
        )
        .param(
            Param::number("risk_free_rate").default(0.0).describe(
                "Annual risk-free rate as a percent (e.g. 2 means 2% per year), used for the Sharpe ratio numerator. Default 0.",
            ),
        )
        .param(
            Param::number("target_return").default(0.0).describe(
                "Sortino minimum acceptable return (MAR) as an annual percent (e.g. 0 or 5). Returns below it count as downside. Default 0.",
            ),
        )
        .param(
            Param::boolean("has_header").default(false).describe(
                "Skip the first line before parsing when your pasted series starts with a column label. Default false.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ReturnsRiskAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/returns-risk-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Annualized return, volatility, Sharpe and Sortino from a returns series",
    skill(
        description = "Compute performance and risk metrics from a series of periodic investment returns: count, per-period mean, cumulative and geometric annualized return, annualized volatility (sample standard deviation × √periods), downside deviation, max drawdown, Sharpe, Sortino and Calmar ratios, plus best/worst period and the share of positive periods. Returns are decimals (0.012) or percents (1.2%). Configure periods_per_year (252 daily, 52 weekly, 12 monthly, 4 quarterly, 1 annual), an annual risk_free_rate percent for Sharpe, and a target_return percent as the Sortino minimum acceptable return. Runs locally. Educational only, not financial advice.",
        parameters = schema_json()
    ),
)]
impl ReturnsRiskAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "returns-risk-analyzer", |a: Args| {
            let ppy: f64 = a
                .periods_per_year
                .trim()
                .parse()
                .map_err(|_| SkillError::InvalidArgs("periods_per_year must be a number".into()))?;
            analyze(&a.returns, ppy, a.risk_free_rate, a.target_return, a.has_header)
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
                    "returns": { "type": "string", "description": "The periodic return series, one return per line or separated by commas/spaces. Each value is a decimal (0.012) or a percent with a % sign (1.2%). Needs at least 2 returns." },
                    "periods_per_year": { "type": "string", "enum": ["252", "52", "26", "12", "4", "1"], "default": "252", "description": "How many return periods make up a year, used to annualize: 252 daily, 52 weekly, 26 biweekly, 12 monthly, 4 quarterly, 1 annual. Default 252 (daily)." },
                    "risk_free_rate": { "type": "number", "default": 0.0, "description": "Annual risk-free rate as a percent (e.g. 2 means 2% per year), used for the Sharpe ratio numerator. Default 0." },
                    "target_return": { "type": "number", "default": 0.0, "description": "Sortino minimum acceptable return (MAR) as an annual percent (e.g. 0 or 5). Returns below it count as downside. Default 0." },
                    "has_header": { "type": "boolean", "default": false, "description": "Skip the first line before parsing when your pasted series starts with a column label. Default false." }
                },
                "required": ["returns"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
