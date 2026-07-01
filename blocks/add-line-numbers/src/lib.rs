//! gizza-ai/add-line-numbers — chat skill block on the shared tool abstraction.
//! Prefixes every line of text with a sequential line number (like `nl` /
//! `cat -n`), with a configurable start, step, separator, alignment, and an
//! optional skip-blank-lines mode. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_add_line_numbers_core::{add_numbers, Pad};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_start")]
    start: i64,
    #[serde(default = "default_step")]
    step: i64,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_pad")]
    pad: String,
    #[serde(default)]
    number_nonblank: bool,
}

fn default_start() -> i64 {
    1
}
fn default_step() -> i64 {
    1
}
fn default_separator() -> String {
    "\t".to_string()
}
fn default_pad() -> String {
    "spaces".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose lines should be numbered (split on newlines)."),
        )
        .param(
            Param::integer("start")
                .default(1)
                .describe("The number given to the first line. Default 1."),
        )
        .param(
            Param::integer("step")
                .default(1)
                .min(1.0)
                .describe("The increment between consecutive line numbers. Default 1."),
        )
        .param(
            Param::string("separator")
                .default("\t")
                .describe("The string inserted between each number and the line content. Default a tab."),
        )
        .param(
            Param::enumv("pad", ["none", "spaces", "zeros"])
                .default("spaces")
                .describe("Align numbers to a common width: spaces (right-align), zeros (zero-pad), or none."),
        )
        .param(
            Param::boolean("number_nonblank")
                .default(false)
                .describe("When true, blank (whitespace-only) lines keep no number and don't consume one (like `cat -b`)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/add-line-numbers",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Prefix every line with a sequential line number",
    skill(
        description = "Prefix every line of text with a sequential line number, like `nl` or `cat -n`. Set start to the first line's number (default 1) and step to the increment between numbers (default 1). separator is placed between the number and the line content (default a tab). pad aligns numbers to a common width: spaces (right-align, default), zeros (zero-pad), or none. Set number_nonblank=true to skip blank lines (they keep no number and don't consume one, like `cat -b`). Returns the numbered text plus total/numbered line counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "add-line-numbers", |a: Args| {
            let pad = Pad::parse(&a.pad).map_err(SkillError::InvalidArgs)?;
            add_numbers(&a.text, a.start, a.step, &a.separator, pad, a.number_nonblank)
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text whose lines should be numbered (split on newlines)." },
                    "start": { "type": "integer", "default": 1, "description": "The number given to the first line. Default 1." },
                    "step": { "type": "integer", "minimum": 1, "default": 1, "description": "The increment between consecutive line numbers. Default 1." },
                    "separator": { "type": "string", "default": "\t", "description": "The string inserted between each number and the line content. Default a tab." },
                    "pad": { "type": "string", "enum": ["none", "spaces", "zeros"], "default": "spaces", "description": "Align numbers to a common width: spaces (right-align), zeros (zero-pad), or none." },
                    "number_nonblank": { "type": "boolean", "default": false, "description": "When true, blank (whitespace-only) lines keep no number and don't consume one (like `cat -b`)." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
