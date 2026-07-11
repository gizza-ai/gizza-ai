//! gizza-ai/slack-export-reader — turn a Slack workspace export (a ZIP of
//! `users.json`, `channels.json` and per-channel `YYYY-MM-DD.json` files) into a
//! readable Markdown or HTML transcript.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref) →
//! `core::render` (pure-Rust zip + serde_json) → flat JSON the LLM reads
//! directly (format, channel/message counts, and the transcript content).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a binary file input with text output fits
//! neither the pure-text page nor the ffmpeg file→media page shape — the
//! no-page file-input pattern, like unzip / epub-to-markdown / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_slack_export_reader_core::{render, Format, Options};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB — exports are mostly small JSON
const MAX_OUTPUT_CHARS: usize = 2_000_000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    date: Option<String>,
}

fn default_format() -> String {
    "markdown".to_string()
}

#[derive(Serialize)]
struct Resp {
    format: String,
    channels: usize,
    messages: usize,
    content: String,
    chars: usize,
    truncated: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("format", ["markdown", "html"])
                .default("markdown")
                .describe(
                    "Output format: markdown (default, `## #channel` headings with bold authors \
                     and resolved links) or html (a standalone, styled transcript document).",
                ),
        )
        .param(
            Param::string("channel")
                .describe(
                    "Optional: only include this channel (case-insensitive, a leading # is \
                     ignored, e.g. `general`). Omit to include every channel in the export.",
                ),
        )
        .param(
            Param::string("date")
                .describe(
                    "Optional: only include this day, as YYYY-MM-DD (e.g. `2021-01-01`), matching \
                     the export's per-day files. Omit to include every day.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SlackExportReader;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/slack-export-reader",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a Slack export ZIP into a readable Markdown or HTML transcript",
    requires = ["wafer-run/network"],
    skill(
        description = "Turn a Slack workspace export (the ZIP of users.json, channels.json and \
            per-channel YYYY-MM-DD.json message files that Slack's 'Export data' produces) into a \
            readable Markdown (default) or HTML transcript. Each message shows the author's display \
            name (resolved from users.json), a UTC timestamp, and text with Slack's markup rewritten \
            — user/channel mentions (<@U…>, <#C…|name>), <!here>/<!channel> commands and \
            <url|label> links become readable. Optional channel (case-insensitive name) and date \
            (YYYY-MM-DD) filters narrow the output; returns the transcript plus channel and message \
            counts. Provide the export as url (HTTP/HTTPS) or ref from a prior tool call. Runs \
            locally — the archive never leaves the device.",
        parameters = schema_json()
    ),
)]
impl SlackExportReader {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn clip_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() > max_chars {
        (text.chars().take(max_chars).collect(), true)
    } else {
        (text.to_string(), false)
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("slack-export-reader")?;
    let format = Format::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let opts = Options {
        format,
        channel: args.channel.filter(|s| !s.trim().is_empty()),
        date: args.date.filter(|s| !s.trim().is_empty()),
    };
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let transcript = render(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let (content, truncated) = clip_chars(&transcript.content, MAX_OUTPUT_CHARS);
    let chars = content.chars().count();

    let resp = Resp {
        format: transcript.format,
        channels: transcript.channels,
        messages: transcript.messages,
        content,
        chars,
        truncated,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize slack-export-reader response: {e}")))
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["markdown", "html"], "default": "markdown", "description": "Output format: markdown (default, `## #channel` headings with bold authors and resolved links) or html (a standalone, styled transcript document)." },
                    "channel": { "type": "string", "description": "Optional: only include this channel (case-insensitive, a leading # is ignored, e.g. `general`). Omit to include every channel in the export." },
                    "date": { "type": "string", "description": "Optional: only include this day, as YYYY-MM-DD (e.g. `2021-01-01`), matching the export's per-day files. Omit to include every day." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
