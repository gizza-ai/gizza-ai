//! gizza-ai/portfolio-allocation — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Breaks pasted holdings
//! into chart-ready allocation percentages.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    group_by: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    top_n: usize,
    #[serde(default)]
    currency: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("Holdings pasted as CSV or tab-separated rows: label, value, optional asset class, optional sector, optional account. A header row is allowed. Values may be currency amounts, thousands-separated numbers, or shares @ price."))
        .param(Param::enumv("group_by", ["asset", "holding", "sector", "account"]).default("asset").describe("Which dimension to allocate by: asset class (default), individual holding, sector, or account."))
        .param(Param::enumv("sort", ["value", "label"]).default("value").describe("Sort slices by descending value (default) or alphabetically by label. When top_n is used, the largest slices are kept before any label sort."))
        .param(Param::integer("top_n").default(0).min(0.0).max(1000.0).describe("Keep only the largest N slices and fold the rest into Other. Use 0 (default) to show every slice."))
        .param(Param::string("currency").default("$").describe("Currency symbol or prefix to display before values, such as $, €, £, USD, or blank for no prefix."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/portfolio-allocation",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Break holdings into chart-ready allocation percentages.",
    skill(
        description = "Break a pasted portfolio holdings list into allocation percentages by asset class, holding, sector, or account. Accepts CSV or tab-separated rows: label, value, optional asset class, optional sector, optional account. Values may be currency amounts, thousands-separated numbers, or shares @ price. Returns a text table with value, percent, proportional bars, holding counts, and a concentration score.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "portfolio-allocation", |a: Args| {
            let group_by = gizza_ai_portfolio_allocation_core::parse_group_by(&a.group_by)
                .map_err(SkillError::InvalidArgs)?;
            let sort = gizza_ai_portfolio_allocation_core::parse_sort(&a.sort)
                .map_err(SkillError::InvalidArgs)?;
            gizza_ai_portfolio_allocation_core::format_table(
                &a.input,
                group_by,
                sort,
                a.top_n,
                &a.currency,
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
                "input":{"type":"string","description":"Holdings pasted as CSV or tab-separated rows: label, value, optional asset class, optional sector, optional account. A header row is allowed. Values may be currency amounts, thousands-separated numbers, or shares @ price."},
                "group_by":{"type":"string","enum":["asset","holding","sector","account"],"default":"asset","description":"Which dimension to allocate by: asset class (default), individual holding, sector, or account."},
                "sort":{"type":"string","enum":["value","label"],"default":"value","description":"Sort slices by descending value (default) or alphabetically by label. When top_n is used, the largest slices are kept before any label sort."},
                "top_n":{"type":"integer","default":0,"minimum":0,"maximum":1000,"description":"Keep only the largest N slices and fold the rest into Other. Use 0 (default) to show every slice."},
                "currency":{"type":"string","default":"$","description":"Currency symbol or prefix to display before values, such as $, €, £, USD, or blank for no prefix."}
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
