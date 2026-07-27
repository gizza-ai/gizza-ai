//! gizza-ai/remove-empty-lines — delete blank or whitespace-only lines,
//! compacting the text. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_remove_empty_lines_core::{process, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_true")]
    whitespace_only: bool,
    #[serde(default)]
    trim_lines: bool,
}

fn default_mode() -> String {
    "remove".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to compact. Blank and (by default) whitespace-only lines are deleted."),
        )
        .param(
            Param::enumv("mode", ["remove", "collapse"])
                .default("remove")
                .describe("remove = delete every empty line (no gaps); collapse = reduce runs of 2+ consecutive empty lines to a single blank line (keeps paragraph spacing). Default remove."),
        )
        .param(
            Param::boolean("whitespace_only")
                .default(true)
                .describe("Also treat lines containing only spaces/tabs (or any whitespace) as empty, not just literally-empty lines. Default true."),
        )
        .param(
            Param::boolean("trim_lines")
                .default(false)
                .describe("Trim leading and trailing whitespace from each kept line. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/remove-empty-lines",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Delete blank or whitespace-only lines, compacting text",
    skill(
        description = "Remove empty lines from text, compacting it. mode=remove (default) deletes every blank line; mode=collapse reduces runs of 2+ consecutive blank lines to a single blank line. whitespace_only=true (default) also deletes lines that contain only spaces/tabs; trim_lines=true trims leading/trailing whitespace from each kept line. Returns the compacted text plus total/removed/kept line counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "remove-empty-lines", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            Ok(process(&a.text, mode, a.whitespace_only, a.trim_lines))
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
                    "text": { "type": "string", "description": "The text to compact. Blank and (by default) whitespace-only lines are deleted." },
                    "mode": { "type": "string", "enum": ["remove", "collapse"], "default": "remove", "description": "remove = delete every empty line (no gaps); collapse = reduce runs of 2+ consecutive empty lines to a single blank line (keeps paragraph spacing). Default remove." },
                    "whitespace_only": { "type": "boolean", "default": true, "description": "Also treat lines containing only spaces/tabs (or any whitespace) as empty, not just literally-empty lines. Default true." },
                    "trim_lines": { "type": "boolean", "default": false, "description": "Trim leading and trailing whitespace from each kept line. Default false." }
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
