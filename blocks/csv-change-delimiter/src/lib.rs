//! gizza-ai/csv-change-delimiter — re-save CSV/DSV with a different separator.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_change_delimiter_core::change_delimiter;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_comma")]
    from: String,
    #[serde(default = "default_tab")]
    to: String,
}
fn default_comma() -> String { ",".to_string() }
fn default_tab() -> String { "tab".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV/DSV text to re-delimit."))
        .param(Param::string("from").default(",").describe("Current field separator: a single char, or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
        .param(Param::string("to").default("tab").describe("Target field separator: a single char, or 'comma'/'tab'/'semicolon'/'pipe'. Default 'tab'."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvChangeDelimiter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-change-delimiter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Change a CSV's field separator",
    skill(
        description = "Re-save delimited data with a different field separator, fixing quoting for the new delimiter (fields containing it get quoted; fields that no longer need quotes are unquoted). `from`/`to` are a single char or one of 'comma'/'tab'/'semicolon'/'pipe'. Defaults: from ',', to 'tab'.",
        parameters = schema_json()
    )
)]
impl CsvChangeDelimiter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-change-delimiter", |a: Args| {
            change_delimiter(&a.data, &a.from, &a.to).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The CSV/DSV text to re-delimit." },
                    "from": { "type": "string", "default": ",", "description": "Current field separator: a single char, or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "to":   { "type": "string", "default": "tab", "description": "Target field separator: a single char, or 'comma'/'tab'/'semicolon'/'pipe'. Default 'tab'." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
