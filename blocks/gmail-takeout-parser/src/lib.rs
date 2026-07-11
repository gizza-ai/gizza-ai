//! gizza-ai/gmail-takeout-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Turns a Google Takeout
//! Gmail mbox export into a CSV/JSON table of messages, one row each, keeping the
//! Gmail labels. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    include_body: bool,
    #[serde(default = "default_snippet_chars")]
    snippet_chars: u32,
}

fn default_format() -> String {
    "csv".into()
}
fn default_snippet_chars() -> u32 {
    200
}

/// Single source for the chat schema (and CLI). Parsing is deterministic: the mbox
/// is split on `From ` postmark lines and each message is read with `mail-parser`;
/// the Gmail-specific `X-Gmail-Labels` header is preserved as a column.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The contents of a Google Takeout Gmail .mbox export (or a single RFC 5322 message). Messages are split on the `From ` postmark lines; each becomes one row with date, from, to, cc, subject, Gmail labels, and message-id columns."),
        )
        .param(
            Param::enumv("format", ["csv", "json"])
                .default("csv")
                .describe("Output format. 'csv' returns an RFC-4180 table (header row + one row per message). 'json' returns an array of message objects. Default 'csv'."),
        )
        .param(
            Param::boolean("include_body")
                .default(false)
                .describe("Add a plain-text body snippet column ('snippet'). Off by default to keep the table compact."),
        )
        .param(
            Param::integer("snippet_chars")
                .default(200)
                .min(0.0)
                .max(2000.0)
                .describe("Maximum characters kept in the body snippet when include_body is on. 0 means the full body (no cap). Default 200."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gmail-takeout-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a Gmail Takeout mbox into a CSV/JSON table",
    skill(
        description = "Parse a Google Takeout Gmail mbox export into a clean table of messages, one row per message, as CSV or JSON. Each row has the standardized date (ISO-8601), from, to, cc, subject, the Gmail labels (from the X-Gmail-Labels header), and the message-id; an optional length-capped body snippet column can be added. The mbox is split on `From ` postmark lines and parsed with a pure-Rust RFC 5322 parser — no LLM, no upload, no attachment extraction.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gmail-takeout-parser", |a: Args| {
            gizza_ai_gmail_takeout_parser_core::run(
                &a.input,
                &a.format,
                a.include_body,
                a.snippet_chars as usize,
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
                    "input": { "type": "string", "description": "The contents of a Google Takeout Gmail .mbox export (or a single RFC 5322 message). Messages are split on the `From ` postmark lines; each becomes one row with date, from, to, cc, subject, Gmail labels, and message-id columns." },
                    "format": { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Output format. 'csv' returns an RFC-4180 table (header row + one row per message). 'json' returns an array of message objects. Default 'csv'." },
                    "include_body": { "type": "boolean", "default": false, "description": "Add a plain-text body snippet column ('snippet'). Off by default to keep the table compact." },
                    "snippet_chars": { "type": "integer", "default": 200, "minimum": 0, "maximum": 2000, "description": "Maximum characters kept in the body snippet when include_body is on. 0 means the full body (no cap). Default 200." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
