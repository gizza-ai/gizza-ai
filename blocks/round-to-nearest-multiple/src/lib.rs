//! gizza-ai/round-to-nearest-multiple — chat skill block on the shared tool abstraction.
//! Rounds each numeric cell of a CSV to the nearest multiple of a chosen step (0.25, 5,
//! 1000, …) using a selectable rounding mode. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_round_to_nearest_multiple_core::round_csv;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_step")]
    step: f64,
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
fn default_step() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text whose numeric cells should be rounded to the nearest multiple."))
        .param(Param::number("step").default(1.0).describe("The multiple to round each value to; must be greater than 0. Examples: 0.25 (quarters), 0.05 (nickels), 5, 100, 1000. Default 1 (round to whole numbers)."))
        .param(Param::enumv("mode", ["half_up", "half_down", "half_even", "ceil", "floor", "truncate"]).default("half_up").describe("How to round to the multiple. 'half_up' = nearest, ties away from zero (classical / Excel MROUND, 2.5→3); 'half_down' = nearest, ties toward zero (2.5→2); 'half_even' = nearest, ties to the even multiple (banker's, 2.5→2, 3.5→4); 'ceil' = always up to the next multiple; 'floor' = always down to the previous multiple; 'truncate' = toward zero. Default 'half_up'."))
        .param(Param::string("columns").default("").describe("Comma-separated columns to round: 1-based indices and/or header names (e.g. 'price,3'). Empty rounds every numeric cell in every column. Default empty."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it unrounded and allow selecting columns by name. Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
        .param(Param::boolean("trailing_zeros").default(false).describe("Pad every rounded cell to the step's own decimal places (e.g. step 0.25 → 1.00, 1.25, 1.50). Default false (natural, no padding)."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct RoundToNearestMultiple;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/round-to-nearest-multiple",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Round the numeric cells of a CSV to the nearest multiple of a step (0.25, 5, 1000, …)",
    skill(
        description = "Round each numeric cell of a CSV to the nearest multiple of a chosen step (e.g. 0.25, 0.05, 5, 100, 1000) using a selectable rounding mode: half_up (nearest, ties away from zero — classical MROUND), half_down (nearest, ties toward zero), half_even (banker's), ceil (always up to the next multiple), floor (always down), or truncate (toward zero). Round every numeric column or a subset chosen by 1-based index and/or header name. A first-row header is kept unrounded. Non-numeric cells (text, currency symbols, thousands separators) pass through unchanged. Rounding of the value÷step quotient is done with exact integer arithmetic on the digits you typed, so halfway values are broken deterministically. Optionally pad results to the step's decimal places.",
        parameters = schema_json()
    ),
)]
impl RoundToNearestMultiple {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "round-to-nearest-multiple", |a: Args| {
            let mode = if a.mode.is_empty() { "half_up".to_string() } else { a.mode };
            let delimiter = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            round_csv(
                &a.data,
                a.step,
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
                    "data":           { "type": "string", "description": "The CSV text whose numeric cells should be rounded to the nearest multiple." },
                    "step":           { "type": "number", "default": 1.0, "description": "The multiple to round each value to; must be greater than 0. Examples: 0.25 (quarters), 0.05 (nickels), 5, 100, 1000. Default 1 (round to whole numbers)." },
                    "mode":           { "type": "string", "enum": ["half_up", "half_down", "half_even", "ceil", "floor", "truncate"], "default": "half_up", "description": "How to round to the multiple. 'half_up' = nearest, ties away from zero (classical / Excel MROUND, 2.5→3); 'half_down' = nearest, ties toward zero (2.5→2); 'half_even' = nearest, ties to the even multiple (banker's, 2.5→2, 3.5→4); 'ceil' = always up to the next multiple; 'floor' = always down to the previous multiple; 'truncate' = toward zero. Default 'half_up'." },
                    "columns":        { "type": "string", "default": "", "description": "Comma-separated columns to round: 1-based indices and/or header names (e.g. 'price,3'). Empty rounds every numeric cell in every column. Default empty." },
                    "header":         { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it unrounded and allow selecting columns by name. Default true." },
                    "delimiter":      { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "trailing_zeros": { "type": "boolean", "default": false, "description": "Pad every rounded cell to the step's own decimal places (e.g. step 0.25 → 1.00, 1.25, 1.50). Default false (natural, no padding)." }
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
