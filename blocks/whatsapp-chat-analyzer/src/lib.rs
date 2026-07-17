//! gizza-ai/whatsapp-chat-analyzer — parse an exported WhatsApp chat and report
//! per-person message counts, busiest hours/days, and word + emoji frequency.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_whatsapp_chat_analyzer_core::{analyze, DateFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    chat: String,
    #[serde(default = "default_date_format")]
    date_format: String,
    #[serde(default = "default_top")]
    top: u32,
    #[serde(default = "default_min_word_length")]
    min_word_length: u32,
    #[serde(default = "default_true")]
    ignore_stopwords: bool,
}
fn default_date_format() -> String {
    "auto".into()
}
fn default_top() -> u32 {
    10
}
fn default_min_word_length() -> u32 {
    3
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("chat")
                .required()
                .describe("The exported WhatsApp chat text. Open the exported _chat.txt (iOS) or the .txt export (Android) and paste its full contents."),
        )
        .param(
            Param::enumv("date_format", ["auto", "dmy", "mdy", "ymd"])
                .default("auto")
                .describe("How to read ambiguous numeric dates for the busiest-days breakdown: auto (detect), dmy (day/month/year), mdy (month/day/year), ymd (ISO year-month-day). Default auto."),
        )
        .param(
            Param::integer("top")
                .min(0.0)
                .max(1000.0)
                .default(10)
                .describe("How many entries to list in the top-words and top-emoji rankings (0 = all). Default 10."),
        )
        .param(
            Param::integer("min_word_length")
                .min(1.0)
                .max(50.0)
                .default(3)
                .describe("Ignore words shorter than this many characters in the word ranking (1 = keep all). Default 3."),
        )
        .param(
            Param::boolean("ignore_stopwords")
                .default(true)
                .describe("When true (default), drop common English filler words (the, and, you, lol, …) from the word ranking."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WhatsappChatAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/whatsapp-chat-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Analyze an exported WhatsApp chat for stats, busiest times, and top words/emoji",
    skill(
        description = "Parse an exported WhatsApp chat (paste the _chat.txt from an iOS export or the .txt from an Android export) and return a plain-text report: total messages, participants, words, media and link counts; messages per person with each person's share; the busiest hours of the day and busiest days of the week; and the most-used words and emoji. Handles both the iOS bracketed format ([2024-01-05, 21:07:33] Alice: …) and the Android dash format (05/01/2024, 21:07 - Alice: …), folds multi-line messages, and skips system/notification lines. Options: date_format (auto/dmy/mdy/ymd for ambiguous dates), top (how many words/emoji to list), min_word_length, and ignore_stopwords. Fully local and deterministic — no AI model, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl WhatsappChatAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "whatsapp-chat-analyzer", |a: Args| {
            analyze(
                &a.chat,
                DateFormat::parse(&a.date_format),
                a.top as usize,
                a.min_word_length as usize,
                a.ignore_stopwords,
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
                    "chat": { "type": "string", "description": "The exported WhatsApp chat text. Open the exported _chat.txt (iOS) or the .txt export (Android) and paste its full contents." },
                    "date_format": { "type": "string", "enum": ["auto", "dmy", "mdy", "ymd"], "default": "auto", "description": "How to read ambiguous numeric dates for the busiest-days breakdown: auto (detect), dmy (day/month/year), mdy (month/day/year), ymd (ISO year-month-day). Default auto." },
                    "top": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 10, "description": "How many entries to list in the top-words and top-emoji rankings (0 = all). Default 10." },
                    "min_word_length": { "type": "integer", "minimum": 1, "maximum": 50, "default": 3, "description": "Ignore words shorter than this many characters in the word ranking (1 = keep all). Default 3." },
                    "ignore_stopwords": { "type": "boolean", "default": true, "description": "When true (default), drop common English filler words (the, and, you, lol, …) from the word ranking." }
                },
                "required": ["chat"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
