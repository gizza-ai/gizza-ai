//! gizza-ai/podcast-feed-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_podcast_feed_parser_core::{to_json, Options, Order};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    feed: String,
    #[serde(default)]
    limit: i64,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default)]
    include_descriptions: bool,
}
fn default_order() -> String {
    "feed".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("feed")
                .required()
                .describe("The podcast RSS/XML feed to parse, pasted as text. Accepts RSS 2.0 (with the iTunes podcast namespace) and Atom 1.0 feeds."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .describe("Maximum number of episodes to return. 0 (default) returns every episode."),
        )
        .param(
            Param::enumv("order", ["feed", "newest", "oldest"])
                .default("feed")
                .describe("Episode order: 'feed' keeps the feed's original order (usually newest first), 'newest' sorts by publish date descending, 'oldest' ascending. Default 'feed'."),
        )
        .param(
            Param::boolean("include_descriptions")
                .default(false)
                .describe("Include each episode's plain-text description/summary in the output. Default false (kept out to keep the list compact)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/podcast-feed-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a podcast RSS/XML feed into a clean episode list",
    skill(
        description = "Parse a pasted podcast RSS/XML feed into a clean, structured episode list. Accepts RSS 2.0 feeds (including the iTunes podcast namespace) and Atom 1.0 feeds. Returns the channel-level metadata (title, description, author, link, cover image, language) plus an `episodes` array where each entry carries the title, publish date (normalised to RFC 3339 in `published`, with the original kept in `published_raw`), duration (normalised `HH:MM:SS` in `duration` plus whole seconds in `duration_seconds`), the audio enclosure URL/MIME-type/byte-size (`audio_url`, `audio_type`, `audio_length_bytes`), GUID, episode page link, season/episode numbers, and explicit flag. Empty/optional fields are omitted. Use `limit` to cap the number of episodes (0 = all), `order` to sort by publish date ('feed' keeps original order, 'newest', or 'oldest'), and `include_descriptions` to add each episode's plain-text summary. Fully local and deterministic — no network fetch, no AI model: paste the feed XML you already have.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "podcast-feed-parser", |a: Args| {
            let opt = Options {
                limit: a.limit.max(0) as usize,
                order: Order::parse(&a.order),
                include_descriptions: a.include_descriptions,
            };
            to_json(&a.feed, &opt).map_err(SkillError::InvalidArgs)
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
            r##"{
                "type": "object",
                "properties": {
                    "feed": { "type": "string", "description": "The podcast RSS/XML feed to parse, pasted as text. Accepts RSS 2.0 (with the iTunes podcast namespace) and Atom 1.0 feeds." },
                    "limit": { "type": "integer", "minimum": 0, "default": 0, "description": "Maximum number of episodes to return. 0 (default) returns every episode." },
                    "order": { "type": "string", "enum": ["feed", "newest", "oldest"], "default": "feed", "description": "Episode order: 'feed' keeps the feed's original order (usually newest first), 'newest' sorts by publish date descending, 'oldest' ascending. Default 'feed'." },
                    "include_descriptions": { "type": "boolean", "default": false, "description": "Include each episode's plain-text description/summary in the output. Default false (kept out to keep the list compact)." }
                },
                "required": ["feed"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
