//! gizza-ai/youtube-id-extractor — pull the video ID (and start offset) out of any YouTube link.
//!
//! Thin chat-skill wrapper around `gizza-ai-youtube-id-extractor-core`. The chat schema is
//! derived from `descriptor()` (single source — shared across chat + CLI + page query-params);
//! the handler delegates to `block_utils::run_skill`. No host calls — pure string parsing inside
//! the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_youtube_id_extractor_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

fn default_format() -> String {
    "text".into()
}
fn default_timestamp() -> String {
    "both".into()
}
fn default_thumbnail() -> String {
    "hqdefault".into()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    urls: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_timestamp")]
    timestamp: String,
    #[serde(default = "default_thumbnail")]
    thumbnail: String,
    #[serde(default = "default_true")]
    canonical: bool,
    #[serde(default)]
    embed: bool,
    #[serde(default)]
    strict: bool,
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("urls")
                .required()
                .describe("YouTube links to parse, one per line (max 200). Accepts watch, youtu.be, shorts, embed, live, /v/, /e/, youtube-nocookie, m./music. subdomains, attribution_link and redirect wrappers, Invidious and Piped front-ends, plus a bare 11-character video ID, a @handle, a UC… channel ID or a playlist ID. Example: https://youtu.be/dQw4w9WgXcQ?t=90"),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv"])
                .default("text")
                .describe("Output shape. 'text' is a labelled report per line, 'json' an array of objects, 'csv' a spreadsheet-ready table with a header row. Default text."),
        )
        .param(
            Param::enumv("timestamp", ["seconds", "clock", "both"])
                .default("both")
                .describe("How a start offset is shown: 'seconds' as 90s, 'clock' as 1:30 (h:mm:ss past an hour), 'both' as 90s (1:30). Default both."),
        )
        .param(
            Param::enumv(
                "thumbnail",
                ["none", "default", "mqdefault", "hqdefault", "sddefault", "maxresdefault"],
            )
                .default("hqdefault")
                .describe("Which i.ytimg.com thumbnail URL to include for each video: none, default (120x90), mqdefault (320x180), hqdefault (480x360), sddefault (640x480) or maxresdefault (1280x720). hqdefault exists for every video; sddefault and maxresdefault 404 on some older uploads. Default hqdefault."),
        )
        .param(
            Param::boolean("canonical")
                .default(true)
                .describe("Include the canonical https://www.youtube.com/watch URL, keeping the playlist and start offset. Default true."),
        )
        .param(
            Param::boolean("embed")
                .default(false)
                .describe("Include the privacy-enhanced https://www.youtube-nocookie.com/embed URL, carrying the playlist and start offset. Default false."),
        )
        .param(
            Param::boolean("strict")
                .default(false)
                .describe("When true, fail the whole run if any line cannot be resolved instead of marking that line invalid and continuing. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct YoutubeIdExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/youtube-id-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the YouTube video ID and start timestamp from any link",
    skill(
        description = "Extract the canonical 11-character video ID, and any start timestamp, from any YouTube-style URL. Handles watch?v=, youtu.be, /shorts/, /embed/, /live/, /v/, /e/, youtube-nocookie.com, the m./music./gaming. subdomains, attribution_link and redirect wrappers, and — unlike most extractors — Invidious and Piped front-ends such as yewtu.be or piped.video, because the same path/query rules are applied to every host. A bare 11-character ID, an @handle, a UC… channel ID or a playlist ID pasted on its own is recognised too. Also reports the playlist ID and index when the link carries them, and can emit the canonical watch URL, an i.ytimg.com thumbnail URL and a youtube-nocookie embed URL. Paste one link per line (max 200) for bulk extraction; choose text, json or csv output. Pure local parsing — nothing is fetched, so titles, durations and whether the video still exists are out of scope.",
        parameters = schema_json()
    )
)]
impl YoutubeIdExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "youtube-id-extractor", |a: Args| {
            extract(
                &a.urls,
                &a.format,
                &a.timestamp,
                &a.thumbnail,
                a.canonical,
                a.embed,
                a.strict,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "urls": { "type": "string", "description": "YouTube links to parse, one per line (max 200). Accepts watch, youtu.be, shorts, embed, live, /v/, /e/, youtube-nocookie, m./music. subdomains, attribution_link and redirect wrappers, Invidious and Piped front-ends, plus a bare 11-character video ID, a @handle, a UC… channel ID or a playlist ID. Example: https://youtu.be/dQw4w9WgXcQ?t=90" },
                    "format": { "type": "string", "enum": ["text", "json", "csv"], "default": "text", "description": "Output shape. 'text' is a labelled report per line, 'json' an array of objects, 'csv' a spreadsheet-ready table with a header row. Default text." },
                    "timestamp": { "type": "string", "enum": ["seconds", "clock", "both"], "default": "both", "description": "How a start offset is shown: 'seconds' as 90s, 'clock' as 1:30 (h:mm:ss past an hour), 'both' as 90s (1:30). Default both." },
                    "thumbnail": { "type": "string", "enum": ["none", "default", "mqdefault", "hqdefault", "sddefault", "maxresdefault"], "default": "hqdefault", "description": "Which i.ytimg.com thumbnail URL to include for each video: none, default (120x90), mqdefault (320x180), hqdefault (480x360), sddefault (640x480) or maxresdefault (1280x720). hqdefault exists for every video; sddefault and maxresdefault 404 on some older uploads. Default hqdefault." },
                    "canonical": { "type": "boolean", "default": true, "description": "Include the canonical https://www.youtube.com/watch URL, keeping the playlist and start offset. Default true." },
                    "embed": { "type": "boolean", "default": false, "description": "Include the privacy-enhanced https://www.youtube-nocookie.com/embed URL, carrying the playlist and start offset. Default false." },
                    "strict": { "type": "boolean", "default": false, "description": "When true, fail the whole run if any line cannot be resolved instead of marking that line invalid and continuing. Default false." }
                },
                "required": ["urls"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
