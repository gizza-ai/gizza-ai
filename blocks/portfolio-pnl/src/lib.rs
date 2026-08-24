//! gizza-ai/portfolio-pnl — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Turns pasted positions
//! into a per-position and portfolio profit/loss report.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    positions: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    fee_percent: f64,
    #[serde(default)]
    tax_rate: f64,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    currency: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("positions").required().describe(
            "Positions pasted as CSV or tab-separated rows: name, quantity, entry price, current price, optional fees, optional 'long' or 'short'. A header row is allowed. Prices may carry a currency symbol, thousands separators, or accounting parentheses. A negative quantity means a short. Example: 'AAPL, 50, 150, 187.50'.",
        ))
        .param(
            Param::enumv("side", ["long", "short"])
                .default("long")
                .describe(
                    "Direction applied to every row that does not end in its own 'long'/'short' word: long (default) gains when the price rises, short gains when it falls.",
                ),
        )
        .param(
            Param::number("fee_percent")
                .default(0.0)
                .min(0.0)
                .max(100.0)
                .describe(
                    "Commission charged on BOTH legs of every position, as a percent of the traded amount (0.1 means 0.1% to open and 0.1% to close). Added on top of any flat fee in a row's 5th column. Default 0.",
                ),
        )
        .param(
            Param::number("tax_rate")
                .default(0.0)
                .min(0.0)
                .max(100.0)
                .describe(
                    "Capital gains tax percent used for the estimated-tax line (15 means 15%). Applied to the portfolio's net gain after losses offset gains, and only when that total is positive. Default 0 (no tax lines shown).",
                ),
        )
        .param(
            Param::enumv("sort", ["input", "pnl", "pnl_percent", "value", "label"])
                .default("input")
                .describe(
                    "Row order: input keeps the pasted order (default), pnl ranks by net gain, pnl_percent by percentage return, value by current market value, label alphabetically.",
                ),
        )
        .param(Param::string("currency").default("$").describe(
            "Currency symbol or prefix shown before amounts, such as $, €, £, USD, or blank for no prefix.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/portfolio-pnl",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Calculate profit/loss per position and overall from entry and current prices.",
    skill(
        description = "Calculate profit and loss for a list of positions from the entry and current prices you provide. Accepts CSV or tab-separated rows: name, quantity, entry price, current price, optional fees, optional long/short. Returns a table with cost basis, market value, gross and net P/L, percentage return and break-even price per position, plus portfolio totals, fees, an optional capital-gains tax estimate, and the best and worst positions. Long and short both supported; no prices are fetched, every number comes from the input.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "portfolio-pnl", |a: Args| {
            let side = gizza_ai_portfolio_pnl_core::Side::parse(&a.side)
                .map_err(SkillError::InvalidArgs)?;
            let sort = gizza_ai_portfolio_pnl_core::Sort::parse(&a.sort)
                .map_err(SkillError::InvalidArgs)?;
            let currency = if a.currency.is_empty() {
                "$".to_string()
            } else {
                a.currency.clone()
            };
            gizza_ai_portfolio_pnl_core::format_table(
                &a.positions,
                side,
                a.fee_percent,
                a.tax_rate,
                sort,
                &currency,
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
              "type":"object",
              "properties":{
                "positions":{"type":"string","description":"Positions pasted as CSV or tab-separated rows: name, quantity, entry price, current price, optional fees, optional 'long' or 'short'. A header row is allowed. Prices may carry a currency symbol, thousands separators, or accounting parentheses. A negative quantity means a short. Example: 'AAPL, 50, 150, 187.50'."},
                "side":{"type":"string","enum":["long","short"],"default":"long","description":"Direction applied to every row that does not end in its own 'long'/'short' word: long (default) gains when the price rises, short gains when it falls."},
                "fee_percent":{"type":"number","default":0.0,"minimum":0,"maximum":100,"description":"Commission charged on BOTH legs of every position, as a percent of the traded amount (0.1 means 0.1% to open and 0.1% to close). Added on top of any flat fee in a row's 5th column. Default 0."},
                "tax_rate":{"type":"number","default":0.0,"minimum":0,"maximum":100,"description":"Capital gains tax percent used for the estimated-tax line (15 means 15%). Applied to the portfolio's net gain after losses offset gains, and only when that total is positive. Default 0 (no tax lines shown)."},
                "sort":{"type":"string","enum":["input","pnl","pnl_percent","value","label"],"default":"input","description":"Row order: input keeps the pasted order (default), pnl ranks by net gain, pnl_percent by percentage return, value by current market value, label alphabetically."},
                "currency":{"type":"string","default":"$","description":"Currency symbol or prefix shown before amounts, such as $, €, £, USD, or blank for no prefix."}
              },
              "required":["positions"],
              "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
