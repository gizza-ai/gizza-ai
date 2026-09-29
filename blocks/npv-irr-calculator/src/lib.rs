//! gizza-ai/npv-irr-calculator — discounted-cash-flow analysis as a chat skill
//! block on the shared tool abstraction. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI and, via manifest.json, the page
//! form); handle() delegates to block_utils::run_skill, which hands the parsed
//! Args to the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    cash_flows: String,
    #[serde(default)]
    initial_investment: f64,
    #[serde(default = "default_discount_rate")]
    discount_rate: f64,
    #[serde(default = "default_period")]
    period: String,
    #[serde(default = "default_timing")]
    timing: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_discount_rate() -> f64 {
    10.0
}
fn default_period() -> String {
    "annual".into()
}
fn default_timing() -> String {
    "end".into()
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

const CASH_FLOWS_DESC: &str = "The cash-flow series, money in as positive numbers and money out as negative, separated by newlines, commas, semicolons, tabs or spaces — for example `-500000, 120000, 180000, 260000`. The first value is period 0 (today, never discounted) unless you fill in initial_investment, in which case the series starts at period 1. Pasted spreadsheet values are tolerated: currency symbols ($ € £ ¥ ₹ ₽), thousands separators (`1,234`), and accounting negatives (`(1,234)` means -1234). Write `12x2500` (or `12*2500`) to repeat a flow twelve times instead of typing it out. Up to 1200 periods including period 0.";
const INITIAL_INVESTMENT_DESC: &str = "Upfront cost entered as a POSITIVE number, for example 500000 for half a million out today. When non-zero it is inserted as the period-0 outflow and the cash_flows series is treated as starting at period 1. Leave it at the default 0 when your series already begins with its own negative period-0 value.";
const DISCOUNT_RATE_DESC: &str = "Required rate of return / cost of capital as a nominal ANNUAL percentage — enter 10 for 10% a year, not 0.1. Default 10. With a non-annual period the per-period rate used for discounting is this figure divided by the periods per year (12% annual over monthly periods = 1% a month). It is also the financing and reinvestment rate used for MIRR. Must be greater than -100.";
const PERIOD_DESC: &str = "Spacing between consecutive cash flows: annual (default), semiannual, quarterly, monthly or weekly. This sets the periods per year (1, 2, 4, 12, 52) used to convert the annual discount rate into a per-period rate and to annualize the IRR and MIRR as (1 + r)^periods_per_year - 1.";
const TIMING_DESC: &str = "When each flow lands inside its period. end (default) is an ordinary annuity: a period-t flow is discounted t periods. begin is an annuity due: every flow from period 1 onwards is discounted one period less (t - 1), which makes the same series worth more. Period 0 is never discounted either way.";
const DECIMALS_DESC: &str = "Decimal places for the money and percentage figures in the summary and table output, 0 to 10. Default 2, which suits currency amounts. Set 0 for whole units on large capital projects. The csv and json outputs keep full precision regardless of this setting.";
const CURRENCY_DESC: &str = "Symbol printed in front of every money amount in the summary and table output, for example $, €, £, ¥, R$ or CHF followed by a space. Default $. Set it to an empty string for plain unlabelled numbers. It is cosmetic only — no exchange rate is applied and no conversion happens.";
const OUTPUT_DESC: &str = "Output shape. summary (default) is the full readable report: NPV, IRR per period and annualized, MIRR, profitability index, plain and discounted payback, inflow/outflow totals, the discounted cash-flow table and any warnings. table returns just the aligned per-period discounted cash-flow table. csv returns that table as comma-separated rows with a header, for a spreadsheet. json returns the whole analysis as a machine-readable object.";

/// Single source for the chat schema, the CLI, and (via
/// `scripts/sync-tool-manifest.py`) the page form's controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("cash_flows")
                .required()
                .describe(CASH_FLOWS_DESC),
        )
        .param(
            Param::number("initial_investment")
                .default(0.0)
                .describe(INITIAL_INVESTMENT_DESC),
        )
        .param(
            Param::number("discount_rate")
                .min(-99.99)
                .max(100.0)
                .default(10.0)
                .describe(DISCOUNT_RATE_DESC),
        )
        .param(
            Param::enumv(
                "period",
                ["annual", "semiannual", "quarterly", "monthly", "weekly"],
            )
            .default("annual")
            .describe(PERIOD_DESC),
        )
        .param(
            Param::enumv("timing", ["end", "begin"])
                .default("end")
                .describe(TIMING_DESC),
        )
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(10.0)
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
    name = "gizza-ai/npv-irr-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Discount a cash-flow series: NPV, IRR, MIRR, profitability index, payback and a per-period discounted table",
    skill(
        description = "Run a discounted-cash-flow analysis on a series of cash flows and say whether it clears the required return. Paste the flows in any spacing — newline, comma, semicolon, tab or space separated — with currency symbols, thousands separators and accounting parentheses tolerated and a `12x2500` shorthand for repeated amounts, optionally with the upfront cost in a separate initial-investment field. Returns the net present value at your discount rate, the internal rate of return found by bracketed bisection (reported per period and annualized), the modified IRR financing and reinvesting at the discount rate, the profitability index, the undiscounted and discounted payback periods interpolated inside the crossing period, inflow and outflow totals, and a per-period table of discount factor, present value and cumulative present value. Options cover annual to weekly period spacing, ordinary end-of-period versus annuity-due beginning-of-period timing, decimal precision, the currency symbol, and summary / table / csv / json output. It warns when the flows change sign more than once so several IRRs are possible, and says so honestly when no IRR exists at all rather than printing a bogus rate. Handles up to 1200 periods, runs locally in pure Rust, and is educational arithmetic only, not financial advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "npv-irr-calculator", |a: Args| {
            gizza_ai_npv_irr_calculator_core::run(
                &a.cash_flows,
                a.initial_investment,
                a.discount_rate,
                &a.period,
                &a.timing,
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
        assert_eq!(props.len(), 8);
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
            "required": ["cash_flows"],
            "properties": {
                "cash_flows": { "type": "string", "description": CASH_FLOWS_DESC },
                "initial_investment": { "type": "number", "default": 0.0, "description": INITIAL_INVESTMENT_DESC },
                "discount_rate": { "type": "number", "minimum": -99.99, "maximum": 100, "default": 10.0, "description": DISCOUNT_RATE_DESC },
                "period": { "type": "string", "enum": ["annual", "semiannual", "quarterly", "monthly", "weekly"], "default": "annual", "description": PERIOD_DESC },
                "timing": { "type": "string", "enum": ["end", "begin"], "default": "end", "description": TIMING_DESC },
                "decimals": { "type": "integer", "minimum": 0, "maximum": 10, "default": 2, "description": DECIMALS_DESC },
                "currency": { "type": "string", "default": "$", "description": CURRENCY_DESC },
                "output": { "type": "string", "enum": ["summary", "table", "csv", "json"], "default": "summary", "description": OUTPUT_DESC }
            }
        });
        assert_eq!(derived, authored);
    }
}
