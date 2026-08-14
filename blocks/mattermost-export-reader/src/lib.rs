//! gizza-ai/mattermost-export-reader — chat skill block on the shared tool
//! abstraction. Parses a Mattermost bulk export (JSONL) into a readable
//! per-channel transcript with author and timestamp resolution, plus a summary.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure Rust → runs on all
//! backends including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mattermost_export_reader_core::{render, Format, Options, Output};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    export: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    user_filter: Option<String>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    until: Option<String>,
    #[serde(default = "yes")]
    include_direct_messages: bool,
    #[serde(default = "yes")]
    include_replies: bool,
    #[serde(default)]
    max_messages: u32,
}

fn default_output() -> String {
    "both".to_string()
}

fn default_format() -> String {
    "text".to_string()
}

fn yes() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("export").required().describe(
                "Paste the entire contents of a Mattermost bulk export .jsonl file (the \
                 import.jsonl inside the archive from `mmctl export create` / the bulk export \
                 tool). It is JSON Lines: one object per line, each tagged with a type such as \
                 version, team, channel, user, post, direct_channel or direct_post.",
            ),
        )
        .param(
            Param::enumv("output", ["transcript", "stats", "both"]).default("both").describe(
                "What to return: transcript (messages grouped per channel, threads nested), stats \
                 (a summary with export counts plus messages per channel and per author), or both \
                 (default).",
            ),
        )
        .param(
            Param::enumv("format", ["text", "markdown", "html", "csv"]).default("text").describe(
                "How to render the result: text (plain readable transcript, default), markdown \
                 (headings, bullets and summary tables), html (escaped paragraphs you can paste \
                 into a page), or csv (machine-readable rows — the transcript becomes \
                 timestamp,channel,author,username,kind,message).",
            ),
        )
        .param(
            Param::string("channel").describe(
                "Optional: keep only this channel, matched case-insensitively against the channel \
                 name or its display name (e.g. `town-square` or `Town Square`; a leading `#` is \
                 ignored). While a channel filter is set, direct messages are excluded.",
            ),
        )
        .param(
            Param::string("user_filter").describe(
                "Optional: keep only messages from this person, matched case-insensitively \
                 against the export username or the resolved display name (e.g. `alice` or \
                 `Alice Anderson`; a leading `@` is ignored). Omit to include everyone.",
            ),
        )
        .param(
            Param::string("since").describe(
                "Optional inclusive start date in YYYY-MM-DD form, compared against each \
                 message's UTC date (e.g. `2024-01-15`). Omit for no lower bound.",
            ),
        )
        .param(
            Param::string("until").describe(
                "Optional inclusive end date in YYYY-MM-DD form, compared against each message's \
                 UTC date (e.g. `2024-01-31`). Omit for no upper bound.",
            ),
        )
        .param(
            Param::boolean("include_direct_messages").default(true).describe(
                "When true (default), direct_post lines are included and labelled by their \
                 members (Direct message: Alice, Bob). Set false to keep channel posts only.",
            ),
        )
        .param(
            Param::boolean("include_replies").default(true).describe(
                "When true (default), thread replies are rendered indented under their parent \
                 post. Set false for root posts only.",
            ),
        )
        .param(
            Param::integer("max_messages").min(0.0).max(500000.0).default(0).describe(
                "Cap on how many messages to render, applied in chronological order after every \
                 filter (0 = no limit). Use it to preview a very large export; the summary reports \
                 the truncation.",
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
    name = "gizza-ai/mattermost-export-reader",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a Mattermost bulk export (JSONL) into a readable per-channel transcript with resolved authors and timestamps",
    skill(
        description = "Parse a Mattermost bulk export (paste the contents of the export's import.jsonl — the JSON Lines file produced by `mmctl export create` or the bulk export tool) into a readable per-channel transcript plus a summary. Each channel post and direct message is rendered with a UTC timestamp, the author's resolved display name (from the export's user lines: nickname, then first + last name, then username) and its channel's display name; thread replies nest under their parent, reactions are aggregated (:thumbsup: x2) and attachments become [attachment: path] placeholders. The summary reports the export format version, teams, channels, direct conversations, users, custom emoji, message/reaction/attachment counts, the date range, and messages per channel and per author with each share. Options: output (transcript/stats/both), format (text/markdown/html/csv), channel filter, user_filter, since/until date bounds (YYYY-MM-DD, UTC), include_direct_messages, include_replies and max_messages. Fully local and deterministic — no AI model, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mattermost-export-reader", |a: Args| {
            let opts = Options {
                output: Output::parse(&a.output),
                format: Format::parse(&a.format).map_err(SkillError::InvalidArgs)?,
                channel: a.channel.filter(|s| !s.trim().is_empty()),
                user_filter: a.user_filter.filter(|s| !s.trim().is_empty()),
                since: a.since.filter(|s| !s.trim().is_empty()),
                until: a.until.filter(|s| !s.trim().is_empty()),
                include_direct_messages: a.include_direct_messages,
                include_replies: a.include_replies,
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
                    "export": { "type": "string", "description": "Paste the entire contents of a Mattermost bulk export .jsonl file (the import.jsonl inside the archive from `mmctl export create` / the bulk export tool). It is JSON Lines: one object per line, each tagged with a type such as version, team, channel, user, post, direct_channel or direct_post." },
                    "output": { "type": "string", "enum": ["transcript", "stats", "both"], "default": "both", "description": "What to return: transcript (messages grouped per channel, threads nested), stats (a summary with export counts plus messages per channel and per author), or both (default)." },
                    "format": { "type": "string", "enum": ["text", "markdown", "html", "csv"], "default": "text", "description": "How to render the result: text (plain readable transcript, default), markdown (headings, bullets and summary tables), html (escaped paragraphs you can paste into a page), or csv (machine-readable rows — the transcript becomes timestamp,channel,author,username,kind,message)." },
                    "channel": { "type": "string", "description": "Optional: keep only this channel, matched case-insensitively against the channel name or its display name (e.g. `town-square` or `Town Square`; a leading `#` is ignored). While a channel filter is set, direct messages are excluded." },
                    "user_filter": { "type": "string", "description": "Optional: keep only messages from this person, matched case-insensitively against the export username or the resolved display name (e.g. `alice` or `Alice Anderson`; a leading `@` is ignored). Omit to include everyone." },
                    "since": { "type": "string", "description": "Optional inclusive start date in YYYY-MM-DD form, compared against each message's UTC date (e.g. `2024-01-15`). Omit for no lower bound." },
                    "until": { "type": "string", "description": "Optional inclusive end date in YYYY-MM-DD form, compared against each message's UTC date (e.g. `2024-01-31`). Omit for no upper bound." },
                    "include_direct_messages": { "type": "boolean", "default": true, "description": "When true (default), direct_post lines are included and labelled by their members (Direct message: Alice, Bob). Set false to keep channel posts only." },
                    "include_replies": { "type": "boolean", "default": true, "description": "When true (default), thread replies are rendered indented under their parent post. Set false for root posts only." },
                    "max_messages": { "type": "integer", "minimum": 0, "maximum": 500000, "default": 0, "description": "Cap on how many messages to render, applied in chronological order after every filter (0 = no limit). Use it to preview a very large export; the summary reports the truncation." }
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
