//! gizza-ai/net-worth-tracker — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Computes net worth from a
//! pasted list of assets and liabilities and breaks it down by category.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    currency: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Assets and liabilities pasted as CSV or tab-separated rows: label, amount, optional type (asset/liability), optional category. A header row is allowed. A row with no type is an asset unless its amount is negative (or wrapped in parentheses), which marks a liability. Amounts may be currency values, thousands-separated numbers, or shares @ price."))
        .param(Param::enumv("sort", ["value", "label"]).default("value").describe("Order categories within each side by descending value (default) or alphabetically by category label."))
        .param(Param::string("currency").default("$").describe("Currency symbol or prefix to display before amounts, such as $, €, £, or blank for no prefix."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/net-worth-tracker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute net worth from assets and liabilities, broken down by category.",
    skill(
        description = "Compute net worth from a pasted list of assets and liabilities. Accepts CSV or tab-separated rows: label, amount, optional type (asset/liability), optional category. A row with no type is an asset unless its amount is negative. Returns a text balance sheet: total assets, total liabilities, net worth, a per-category breakdown of each side with value, percent, proportional bars and item counts, and the debt-to-asset ratio.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "net-worth-tracker", |a: Args| {
            let sort = gizza_ai_net_worth_tracker_core::parse_sort(&a.sort)
                .map_err(SkillError::InvalidArgs)?;
            gizza_ai_net_worth_tracker_core::format_report(&a.input, sort, &a.currency)
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
                "input":{"type":"string","description":"Assets and liabilities pasted as CSV or tab-separated rows: label, amount, optional type (asset/liability), optional category. A header row is allowed. A row with no type is an asset unless its amount is negative (or wrapped in parentheses), which marks a liability. Amounts may be currency values, thousands-separated numbers, or shares @ price."},
                "sort":{"type":"string","enum":["value","label"],"default":"value","description":"Order categories within each side by descending value (default) or alphabetically by category label."},
                "currency":{"type":"string","default":"$","description":"Currency symbol or prefix to display before amounts, such as $, €, £, or blank for no prefix."}
              },
              "required":["input"],
              "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
