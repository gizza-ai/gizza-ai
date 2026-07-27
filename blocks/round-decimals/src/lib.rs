//! gizza-ai/round-decimals — chat skill block on the shared tool abstraction.
//! Rounds the numeric cells of a CSV to a chosen number of decimal places using a
//! selectable rounding mode. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_round_decimals_core::round_csv;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    trailing_zeros: bool,
}
fn default_decimals() -> i64 {
    2
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text whose numeric cells should be rounded."))
        .param(Param::integer("decimals").default(2).min(0.0).max(10.0).describe("Number of decimal places to keep, 0–10. Default 2."))
        .param(Param::enumv("mode", ["half_up", "half_down", "half_even", "ceil", "floor", "truncate"]).default("half_up").describe("Rounding mode. 'half_up' rounds halves away from zero (classical, 2.5→3); 'half_down' rounds halves toward zero (2.5→2); 'half_even' is banker's rounding (2.5→2, 3.5→4); 'ceil' always rounds up; 'floor' always rounds down; 'truncate' drops the extra digits. Default 'half_up'."))
        .param(Param::string("columns").default("").describe("Comma-separated columns to round: 1-based indices and/or header names (e.g. 'price,3'). Empty rounds every numeric cell in every column. Default empty."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it unrounded and allow selecting columns by name. Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
        .param(Param::boolean("trailing_zeros").default(false).describe("Pad every rounded cell to exactly 'decimals' places (e.g. 3 → 3.00). Default false (natural, no padding)."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct RoundDecimals;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/round-decimals",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Round the numeric columns of a CSV to N decimal places with a selectable rounding mode",
    skill(
        description = "Round the numeric cells of a CSV to a chosen number of decimal places (0–10) using a selectable rounding mode: half_up (away from zero, classical), half_down (toward zero), half_even (banker's), ceil, floor, or truncate. Round every numeric column or a subset chosen by 1-based index and/or header name. A first-row header is kept unrounded. Non-numeric cells (text, currency symbols) pass through unchanged. Rounding is done on the decimal digit string, so 1.005 at 2 places is 1.01. Optionally pad results to a fixed number of decimals.",
        parameters = schema_json()
    ),
)]
impl RoundDecimals {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "round-decimals", |a: Args| {
            let mode = if a.mode.is_empty() { "half_up".to_string() } else { a.mode };
            let delimiter = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            round_csv(
                &a.data,
                a.decimals,
                &mode,
                &a.columns,
                a.header,
                &delimiter,
                a.trailing_zeros,
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
                    "data":           { "type": "string", "description": "The CSV text whose numeric cells should be rounded." },
                    "decimals":       { "type": "integer", "default": 2, "minimum": 0, "maximum": 10, "description": "Number of decimal places to keep, 0–10. Default 2." },
                    "mode":           { "type": "string", "enum": ["half_up", "half_down", "half_even", "ceil", "floor", "truncate"], "default": "half_up", "description": "Rounding mode. 'half_up' rounds halves away from zero (classical, 2.5→3); 'half_down' rounds halves toward zero (2.5→2); 'half_even' is banker's rounding (2.5→2, 3.5→4); 'ceil' always rounds up; 'floor' always rounds down; 'truncate' drops the extra digits. Default 'half_up'." },
                    "columns":        { "type": "string", "default": "", "description": "Comma-separated columns to round: 1-based indices and/or header names (e.g. 'price,3'). Empty rounds every numeric cell in every column. Default empty." },
                    "header":         { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it unrounded and allow selecting columns by name. Default true." },
                    "delimiter":      { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "trailing_zeros": { "type": "boolean", "default": false, "description": "Pad every rounded cell to exactly 'decimals' places (e.g. 3 → 3.00). Default false (natural, no padding)." }
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
