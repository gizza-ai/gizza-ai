//! gizza-ai/chat-log-analyzer — parse an IRC or generic chat log and report who
//! talked most, activity by hour and weekday, word frequency, links, and channel
//! events. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_chat_log_analyzer_core::{analyze, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    log: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_top")]
    top: u32,
    #[serde(default = "default_min_word_length")]
    min_word_length: u32,
    #[serde(default = "default_true")]
    ignore_stopwords: bool,
    #[serde(default)]
    exclude_nicks: String,
}
fn default_output() -> String {
    "summary".into()
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
            Param::string("log")
                .required()
                .describe("The chat or IRC log text. Paste the raw lines, e.g. '21:07 <alice> hey', '[21:07:33] <@alice> hey', '2024-01-05 21:07:33 <alice> hey', a tab-separated WeeChat log, or a plain 'alice: hey' transcript. Timestamps are optional. Maximum 5000000 bytes."),
        )
        .param(
            Param::enumv("output", ["summary", "json"])
                .default("summary")
                .describe("Report shape: summary (readable text report with ASCII bar charts) or json (the same numbers as a machine-readable object). Default summary."),
        )
        .param(
            Param::integer("top")
                .min(0.0)
                .max(1000.0)
                .default(10)
                .describe("How many people, words, and link domains to list in each ranking (0 = all). Default 10."),
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
        .param(
            Param::string("exclude_nicks")
                .default("")
                .describe("Comma-separated nicks to leave out of every statistic, for bots and services — e.g. 'gizzabot, travis*'. A trailing * matches by prefix. Empty by default, which counts everyone."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ChatLogAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chat-log-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Analyze a chat or IRC log for who talked most, activity by hour, and word stats",
    skill(
        description = "Parse a chat or IRC log and return a report: total messages, participants, words and characters, the time span covered, who talked most (messages, share, words, characters, words and characters per message), activity by hour of day and by weekday as ASCII bar charts, the most-used words, the links shared with their top domains, and channel events (joins, parts, quits, nick changes, kicks, mode and topic changes, /me actions). Auto-detects the common log dialects — irssi/HexChat/mIRC angle-bracket lines with optional [bracketed] or bare timestamps, dated and ISO timestamps, 12-hour AM/PM times, tab-separated WeeChat columns, and plain 'nick: message' transcripts — so no format option is needed. Event lines are excluded from the message and word stats. Options: output (summary or json), top (how many entries per ranking), min_word_length, ignore_stopwords, and exclude_nicks (comma list, trailing * matches by prefix) to drop bots. Fully local and deterministic — no AI model, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl ChatLogAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "chat-log-analyzer", |a: Args| {
            analyze(
                &a.log,
                OutputFormat::parse(&a.output),
                a.top as usize,
                a.min_word_length as usize,
                a.ignore_stopwords,
                &a.exclude_nicks,
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
                    "log": { "type": "string", "description": "The chat or IRC log text. Paste the raw lines, e.g. '21:07 <alice> hey', '[21:07:33] <@alice> hey', '2024-01-05 21:07:33 <alice> hey', a tab-separated WeeChat log, or a plain 'alice: hey' transcript. Timestamps are optional. Maximum 5000000 bytes." },
                    "output": { "type": "string", "enum": ["summary", "json"], "default": "summary", "description": "Report shape: summary (readable text report with ASCII bar charts) or json (the same numbers as a machine-readable object). Default summary." },
                    "top": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 10, "description": "How many people, words, and link domains to list in each ranking (0 = all). Default 10." },
                    "min_word_length": { "type": "integer", "minimum": 1, "maximum": 50, "default": 3, "description": "Ignore words shorter than this many characters in the word ranking (1 = keep all). Default 3." },
                    "ignore_stopwords": { "type": "boolean", "default": true, "description": "When true (default), drop common English filler words (the, and, you, lol, …) from the word ranking." },
                    "exclude_nicks": { "type": "string", "default": "", "description": "Comma-separated nicks to leave out of every statistic, for bots and services — e.g. 'gizzabot, travis*'. A trailing * matches by prefix. Empty by default, which counts everyone." }
                },
                "required": ["log"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
