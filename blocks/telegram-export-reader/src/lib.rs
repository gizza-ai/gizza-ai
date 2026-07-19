//! gizza-ai/telegram-export-reader — chat skill block on the shared tool
//! abstraction. Parses a Telegram Desktop `result.json` export into a clean
//! transcript plus per-sender message + word statistics. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure Rust → runs on all backends
//! including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_telegram_export_reader_core::{render, Options, Output};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    export: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    include_service_messages: bool,
    #[serde(default)]
    sender_filter: Option<String>,
    #[serde(default)]
    max_messages: u32,
}

fn default_output() -> String {
    "both".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("export").required().describe(
                "Paste the entire contents of result.json from a Telegram Desktop export \
                 (a chat's ⋮ menu → Export chat history → format: Machine-readable JSON, or \
                 Settings → Advanced → Export Telegram data). Single-chat exports, full-account \
                 exports (chats.list), and a bare array of message objects are all accepted.",
            ),
        )
        .param(
            Param::enumv("output", ["transcript", "stats", "both"]).default("both").describe(
                "What to return: transcript (one clean, dated line per message), stats (per-sender \
                 message and word counts with each sender's share, plus top words and emoji), or \
                 both (default).",
            ),
        )
        .param(
            Param::boolean("include_service_messages").default(false).describe(
                "When true, include Telegram service/system lines (group created, members added, \
                 name changed, calls) in the transcript. Default false.",
            ),
        )
        .param(
            Param::string("sender_filter").describe(
                "Optional: keep only messages from this exact display name (case-insensitive, e.g. \
                 `Alice`). Omit to include every sender.",
            ),
        )
        .param(
            Param::integer("max_messages").min(0.0).max(500000.0).default(0).describe(
                "Cap on how many messages to read from the export, applied after the service and \
                 sender filters (0 = no limit). Use it to preview a very large export. Default 0.",
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
    name = "gizza-ai/telegram-export-reader",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a Telegram result.json export into a clean transcript plus per-sender message and word stats",
    skill(
        description = "Parse a Telegram Desktop chat export (paste the contents of result.json — the Machine-readable JSON produced by \"Export chat history\") into a clean, dated transcript and/or per-sender statistics. The transcript renders one line per message ([2021-03-27 14:45:00] Alice: …), flattens formatted/entity text, shows readable placeholders for media ([photo], [sticker 🎉], [voice message], [file: name.pdf]), and can optionally include service lines (group created, members added). The stats report gives total messages, participants, word count, media/service counts, date range, a per-sender leaderboard of message and word counts with each sender's share, and the most-used words and emoji. Options: output (transcript/stats/both), include_service_messages, sender_filter (one person), and max_messages (cap for huge exports). Accepts single-chat exports, full-account exports (chats.list), and a bare message array. Fully local and deterministic — no AI model, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "telegram-export-reader", |a: Args| {
            let opts = Options {
                output: Output::parse(&a.output),
                include_service: a.include_service_messages,
                sender_filter: a.sender_filter,
                max_messages: a.max_messages as usize,
            };
            render(&a.export, &opts).map_err(SkillError::InvalidArgs)
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
                    "export": { "type": "string", "description": "Paste the entire contents of result.json from a Telegram Desktop export (a chat's ⋮ menu → Export chat history → format: Machine-readable JSON, or Settings → Advanced → Export Telegram data). Single-chat exports, full-account exports (chats.list), and a bare array of message objects are all accepted." },
                    "output": { "type": "string", "enum": ["transcript", "stats", "both"], "default": "both", "description": "What to return: transcript (one clean, dated line per message), stats (per-sender message and word counts with each sender's share, plus top words and emoji), or both (default)." },
                    "include_service_messages": { "type": "boolean", "default": false, "description": "When true, include Telegram service/system lines (group created, members added, name changed, calls) in the transcript. Default false." },
                    "sender_filter": { "type": "string", "description": "Optional: keep only messages from this exact display name (case-insensitive, e.g. `Alice`). Omit to include every sender." },
                    "max_messages": { "type": "integer", "minimum": 0, "maximum": 500000, "default": 0, "description": "Cap on how many messages to read from the export, applied after the service and sender filters (0 = no limit). Use it to preview a very large export. Default 0." }
                },
                "required": ["export"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
