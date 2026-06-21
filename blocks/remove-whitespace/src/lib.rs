//! gizza-ai/remove-whitespace — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_remove_whitespace_core::{clean, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    collapse_blank_lines: bool,
}
fn default_mode() -> String {
    "trim".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose whitespace should be cleaned up."),
        )
        .param(
            Param::enumv("mode", ["trim", "collapse", "strip"])
                .default("trim")
                .describe(
                    "How to clean whitespace: 'trim' (default) strips leading/trailing whitespace from each line and removes leading/trailing blank lines while keeping internal spacing; 'collapse' also squashes runs of inline whitespace to a single space; 'strip' removes every whitespace character to produce one dense string.",
                ),
        )
        .param(
            Param::boolean("collapse_blank_lines").default(false).describe(
                "When true, collapse runs of 2+ blank lines down to a single blank line. Ignored when mode is 'strip'.",
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
    name = "gizza-ai/remove-whitespace",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trim, collapse, or strip whitespace from text",
    skill(
        description = "Clean up whitespace in text. mode='trim' (default) strips leading/trailing whitespace from each line and drops leading/trailing blank lines; mode='collapse' also squashes runs of inline spaces/tabs to a single space; mode='strip' removes every whitespace character (including Unicode spaces like NBSP) to produce one dense string. collapse_blank_lines collapses runs of blank lines. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "remove-whitespace", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            Ok(clean(&a.text, mode, a.collapse_blank_lines))
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
                    "text": { "type": "string", "description": "The text whose whitespace should be cleaned up." },
                    "mode": { "type": "string", "enum": ["trim", "collapse", "strip"], "default": "trim", "description": "How to clean whitespace: 'trim' (default) strips leading/trailing whitespace from each line and removes leading/trailing blank lines while keeping internal spacing; 'collapse' also squashes runs of inline whitespace to a single space; 'strip' removes every whitespace character to produce one dense string." },
                    "collapse_blank_lines": { "type": "boolean", "default": false, "description": "When true, collapse runs of 2+ blank lines down to a single blank line. Ignored when mode is 'strip'." }
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
