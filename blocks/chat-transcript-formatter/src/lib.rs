//! gizza-ai/chat-transcript-formatter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_time_format")]
    time_format: String,
    #[serde(default)]
    include_dates: bool,
    #[serde(default)]
    merge_consecutive: bool,
    #[serde(default)]
    blank_line_between: bool,
}

fn default_output_format() -> String {
    "plain".to_string()
}
fn default_time_format() -> String {
    "keep".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The raw chat log or conversation transcript to reformat. Handles WhatsApp exports, IRC/Discord copy-paste, '[HH:MM] Name:' timestamped logs, and plain 'Name: message' lines; unrecognized lines fold into the previous message."))
        .param(Param::enumv("output_format", ["plain", "markdown", "bracketed", "screenplay"]).default("plain").describe("Speaker-label style: 'plain' (Name: message), 'markdown' (**Name:** message), 'bracketed' (<Name> message), or 'screenplay' (NAME: message, uppercased)."))
        .param(Param::enumv("time_format", ["keep", "24h", "12h", "none"]).default("keep").describe("Timestamp handling: 'keep' verbatim, '24h' normalized to HH:MM, '12h' normalized to h:MM AM/PM, or 'none' to drop timestamps."))
        .param(Param::boolean("include_dates").default(false).describe("Keep the date part (from WhatsApp-style exports) alongside the time. Off by default so the output stays compact."))
        .param(Param::boolean("merge_consecutive").default(false).describe("Merge consecutive turns from the same speaker into one block."))
        .param(Param::boolean("blank_line_between").default(false).describe("Put a blank line between turns for a more readable, paragraph-style layout."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chat-transcript-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reformat a messy chat log into a clean, consistent transcript",
    skill(
        description = "Take a raw, inconsistently-formatted chat log or conversation transcript and re-emit it as one uniformly-formatted transcript that preserves speaker, time, and message. It deterministically parses the common line shapes — WhatsApp bracket ('[2023-01-05, 10:04] Alice: hi') and dash ('05/01/2023, 10:04 AM - Alice: hi') exports, bracketed/parenthesized times ('[10:04] Alice:' / '(10:04 AM) <Bob>'), bare leading times ('10:04 Alice:'), IRC/Discord angle form ('<Alice> hi'), and plain 'Name: message' — and folds any unrecognized wrapped line into the previous message. Choose the speaker style with output_format (plain / markdown-bold / IRC angle / screenplay), normalize or drop timestamps with time_format (keep / 24h / 12h / none), keep dates with include_dates, merge consecutive same-speaker turns with merge_consecutive, and add blank lines between turns with blank_line_between. Fully deterministic — no LLM, no network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "chat-transcript-formatter", |a: Args| {
            gizza_ai_chat_transcript_formatter_core::run(
                &a.input,
                &a.output_format,
                &a.time_format,
                a.include_dates,
                a.merge_consecutive,
                a.blank_line_between,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "The raw chat log or conversation transcript to reformat. Handles WhatsApp exports, IRC/Discord copy-paste, '[HH:MM] Name:' timestamped logs, and plain 'Name: message' lines; unrecognized lines fold into the previous message." },
                "output_format": { "type": "string", "enum": ["plain", "markdown", "bracketed", "screenplay"], "default": "plain", "description": "Speaker-label style: 'plain' (Name: message), 'markdown' (**Name:** message), 'bracketed' (<Name> message), or 'screenplay' (NAME: message, uppercased)." },
                "time_format": { "type": "string", "enum": ["keep", "24h", "12h", "none"], "default": "keep", "description": "Timestamp handling: 'keep' verbatim, '24h' normalized to HH:MM, '12h' normalized to h:MM AM/PM, or 'none' to drop timestamps." },
                "include_dates": { "type": "boolean", "default": false, "description": "Keep the date part (from WhatsApp-style exports) alongside the time. Off by default so the output stays compact." },
                "merge_consecutive": { "type": "boolean", "default": false, "description": "Merge consecutive turns from the same speaker into one block." },
                "blank_line_between": { "type": "boolean", "default": false, "description": "Put a blank line between turns for a more readable, paragraph-style layout." }
            },
            "required": ["input"],
            "additionalProperties": false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
