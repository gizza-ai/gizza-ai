//! gizza-ai/log-return-calculator — converts a price/level series into
//! continuously-compounded (log) returns. Thin wrapper; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends. Educational only, not financial advice.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_log_return_calculator_core::summary;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    prices: String,
    #[serde(default)]
    has_header: bool,
    #[serde(default = "default_ppy")]
    periods_per_year: String,
    #[serde(default = "default_unit")]
    unit: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_ppy() -> String {
    "252".to_string()
}
fn default_unit() -> String {
    "percent".to_string()
}
fn default_decimals() -> i64 {
    4
}
fn default_output() -> String {
    "summary".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("prices").required().describe(
                "The price or level series (levels, not returns): one price per line, or a single line of prices separated by commas. Each line may also be 'label,price' (e.g. '2024-01-02,187.15') to name the rows. Currency symbols and thousands separators are accepted. Every price must be greater than 0 because a log return is ln(price ÷ previous price). Needs at least 2 prices, maximum 2000.",
            ),
        )
        .param(
            Param::boolean("has_header").default(false).describe(
                "Skip the first line before parsing, for when the pasted column starts with a header like 'date,close'. Default false.",
            ),
        )
        .param(
            Param::enumv("periods_per_year", ["252", "365", "52", "26", "12", "4", "1"])
                .default("252")
                .describe(
                    "How many prices make up a year, used to annualize the mean return and the volatility: 252 trading days, 365 calendar days, 52 weekly, 26 biweekly, 12 monthly, 4 quarterly, 1 annual. Default 252.",
                ),
        )
        .param(
            Param::enumv("unit", ["percent", "decimal"])
                .default("percent")
                .describe(
                    "How each return is written: percent shows +1.1778%, decimal shows the raw natural-log value +0.011778. Default percent.",
                ),
        )
        .param(
            Param::integer("decimals")
                .default(4)
                .min(0.0)
                .max(10.0)
                .describe("Decimal places for every return in the output, 0 to 10. Default 4."),
        )
        .param(
            Param::enumv("output", ["summary", "table", "csv", "json"])
                .default("summary")
                .describe(
                    "Output shape: summary (headline metrics plus the per-period table), table (just the table), csv (the table as CSV for a spreadsheet), or json (every computed field). Default summary.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-return-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Continuously-compounded log returns and volatility from a price series",
    skill(
        description = "Convert a series of PRICES or levels — closes, index levels, NAVs, exchange rates — into continuously-compounded log returns, ln(p_t / p_{t-1}). Paste one price per line (optionally as 'label,price' like '2024-01-02,187.15'), or a single comma-separated line; currency symbols and thousands separators are accepted and every price must be positive. Returns the per-period log return, the simple return of the same step and a running cumulative log return, plus the total log return (which equals the sum of the periods because log returns are time-additive), the equivalent simple return, the mean log return, the per-period and annualized volatility, the annualized log and simple rates, best and worst periods, and how many periods were positive. Set periods_per_year (252 trading days, 365 calendar, 52 weekly, 26 biweekly, 12 monthly, 4 quarterly, 1 annual) to annualize, unit=percent or decimal, decimals 0-10, and output=summary, table, csv or json. Runs locally. Educational only, not financial advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "log-return-calculator", |a: Args| {
            let ppy: f64 = a.periods_per_year.trim().parse().map_err(|_| {
                SkillError::InvalidArgs(format!(
                    "periods_per_year must be a number, got '{}'",
                    a.periods_per_year
                ))
            })?;
            summary(&a.prices, a.has_header, ppy, &a.unit, a.decimals, &a.output)
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
                    "prices": { "type": "string", "description": "The price or level series (levels, not returns): one price per line, or a single line of prices separated by commas. Each line may also be 'label,price' (e.g. '2024-01-02,187.15') to name the rows. Currency symbols and thousands separators are accepted. Every price must be greater than 0 because a log return is ln(price ÷ previous price). Needs at least 2 prices, maximum 2000." },
                    "has_header": { "type": "boolean", "default": false, "description": "Skip the first line before parsing, for when the pasted column starts with a header like 'date,close'. Default false." },
                    "periods_per_year": { "type": "string", "enum": ["252", "365", "52", "26", "12", "4", "1"], "default": "252", "description": "How many prices make up a year, used to annualize the mean return and the volatility: 252 trading days, 365 calendar days, 52 weekly, 26 biweekly, 12 monthly, 4 quarterly, 1 annual. Default 252." },
                    "unit": { "type": "string", "enum": ["percent", "decimal"], "default": "percent", "description": "How each return is written: percent shows +1.1778%, decimal shows the raw natural-log value +0.011778. Default percent." },
                    "decimals": { "type": "integer", "default": 4, "minimum": 0, "maximum": 10, "description": "Decimal places for every return in the output, 0 to 10. Default 4." },
                    "output": { "type": "string", "enum": ["summary", "table", "csv", "json"], "default": "summary", "description": "Output shape: summary (headline metrics plus the per-period table), table (just the table), csv (the table as CSV for a spreadsheet), or json (every computed field). Default summary." }
                },
                "required": ["prices"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
