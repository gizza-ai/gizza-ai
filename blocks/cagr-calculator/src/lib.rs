//! gizza-ai/cagr-calculator — compound annual growth rate and period-over-period
//! growth for a series of VALUES (revenue, GDP, price, headcount). Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends. Educational only, not financial advice.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_cagr_calculator_core::summary;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    values: String,
    #[serde(default = "default_period")]
    period: String,
    #[serde(default)]
    years: f64,
    #[serde(default)]
    start_date: String,
    #[serde(default)]
    end_date: String,
    #[serde(default)]
    target_value: f64,
    #[serde(default)]
    has_header: bool,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_period() -> String {
    "annual".to_string()
}
fn default_decimals() -> i64 {
    2
}
fn default_output() -> String {
    "summary".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("values").required().describe(
                "The value series (levels, not returns): one number per line, or a single line of numbers separated by commas. Each line may also be 'label,value' (e.g. '2000,1234') to name the periods. Currency symbols, thousands separators and accounting negatives like (1,234) are accepted. Needs at least 2 values, maximum 2000.",
            ),
        )
        .param(
            Param::enumv("period", ["annual", "quarterly", "monthly", "weekly", "daily"])
                .default("annual")
                .describe(
                    "Spacing between consecutive values, used to work out the elapsed years (annual=1, quarterly=4, monthly=12, weekly=52, daily=365 calendar days per year). Default annual. Ignored when you set years or a date range.",
                ),
        )
        .param(
            Param::number("years").default(0.0).min(0.0).describe(
                "Total elapsed years spanned by the series, overriding the period spacing. Use this for the classic two-value case (e.g. values=100000,150000 with years=5) or for irregularly spaced data. 0 means derive it from period. Default 0.",
            ),
        )
        .param(
            Param::string("start_date").default("").describe(
                "Optional exact start date as YYYY-MM-DD (e.g. 2000-01-31). Combined with end_date it sets the elapsed years to the real day count / 365.25. Used only when years is 0.",
            ),
        )
        .param(
            Param::string("end_date").default("").describe(
                "Optional exact end date as YYYY-MM-DD, paired with start_date. Must be after start_date.",
            ),
        )
        .param(
            Param::number("target_value").default(0.0).min(0.0).describe(
                "Optional goal value. Returns how many more years from the last value it takes to reach it at the computed CAGR. 0 means no projection. Default 0.",
            ),
        )
        .param(
            Param::boolean("has_header").default(false).describe(
                "Skip the first line before parsing, for when the pasted column starts with a header like 'year,gdp'. Default false.",
            ),
        )
        .param(
            Param::integer("decimals")
                .default(2)
                .min(0.0)
                .max(10.0)
                .describe("Decimal places for the percentages in the output, 0 to 10. Default 2."),
        )
        .param(
            Param::enumv("output", ["summary", "table", "csv", "json"])
                .default("summary")
                .describe(
                    "Output shape: summary (headline metrics plus the period-over-period table), table (just the table), csv (the table as CSV for a spreadsheet), or json (every computed field). Default summary.",
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
    name = "gizza-ai/cagr-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compound annual growth rate and period-over-period growth for a value series",
    skill(
        description = "Compute the compound annual growth rate (CAGR) and the period-over-period growth of a series of VALUES — revenue, GDP, price, headcount, subscribers — not a series of returns. Paste one number per line (optionally as 'label,value' like '2000,1234'), or a single comma-separated line; currency symbols, thousands separators and accounting negatives are accepted. Elapsed time comes from the period spacing (annual/quarterly/monthly/weekly/daily), or from an explicit years override, or from an exact start_date/end_date range, in that order of precedence. Returns CAGR, total growth, the growth multiple, absolute change, compound growth per period, the arithmetic mean period growth, best and worst periods, doubling time, an optional years-to-target projection, and a per-period table of change, growth percent and an index rebased to 100. Choose output=summary, table, csv or json. Runs locally. Educational only, not financial advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cagr-calculator", |a: Args| {
            summary(
                &a.values,
                &a.period,
                a.years,
                &a.start_date,
                &a.end_date,
                a.target_value,
                a.has_header,
                a.decimals,
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
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "values": { "type": "string", "description": "The value series (levels, not returns): one number per line, or a single line of numbers separated by commas. Each line may also be 'label,value' (e.g. '2000,1234') to name the periods. Currency symbols, thousands separators and accounting negatives like (1,234) are accepted. Needs at least 2 values, maximum 2000." },
                    "period": { "type": "string", "enum": ["annual", "quarterly", "monthly", "weekly", "daily"], "default": "annual", "description": "Spacing between consecutive values, used to work out the elapsed years (annual=1, quarterly=4, monthly=12, weekly=52, daily=365 calendar days per year). Default annual. Ignored when you set years or a date range." },
                    "years": { "type": "number", "default": 0.0, "minimum": 0, "description": "Total elapsed years spanned by the series, overriding the period spacing. Use this for the classic two-value case (e.g. values=100000,150000 with years=5) or for irregularly spaced data. 0 means derive it from period. Default 0." },
                    "start_date": { "type": "string", "default": "", "description": "Optional exact start date as YYYY-MM-DD (e.g. 2000-01-31). Combined with end_date it sets the elapsed years to the real day count / 365.25. Used only when years is 0." },
                    "end_date": { "type": "string", "default": "", "description": "Optional exact end date as YYYY-MM-DD, paired with start_date. Must be after start_date." },
                    "target_value": { "type": "number", "default": 0.0, "minimum": 0, "description": "Optional goal value. Returns how many more years from the last value it takes to reach it at the computed CAGR. 0 means no projection. Default 0." },
                    "has_header": { "type": "boolean", "default": false, "description": "Skip the first line before parsing, for when the pasted column starts with a header like 'year,gdp'. Default false." },
                    "decimals": { "type": "integer", "default": 2, "minimum": 0, "maximum": 10, "description": "Decimal places for the percentages in the output, 0 to 10. Default 2." },
                    "output": { "type": "string", "enum": ["summary", "table", "csv", "json"], "default": "summary", "description": "Output shape: summary (headline metrics plus the period-over-period table), table (just the table), csv (the table as CSV for a spreadsheet), or json (every computed field). Default summary." }
                },
                "required": ["values"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
