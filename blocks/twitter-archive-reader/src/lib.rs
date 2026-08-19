//! gizza-ai/twitter-archive-reader — chat skill block on the shared tool
//! abstraction. Parses the `tweets.js` file from a Twitter/X data export into a
//! readable transcript plus posting statistics and top tweets.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure Rust → runs on all
//! backends including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_twitter_archive_reader_core::{render, Format, Options, Output, Sort};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tweets: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    until: Option<String>,
    #[serde(default = "yes")]
    include_replies: bool,
    #[serde(default = "yes")]
    include_retweets: bool,
    #[serde(default = "yes")]
    expand_urls: bool,
    #[serde(default = "default_top_count")]
    top_count: u32,
    #[serde(default)]
    max_tweets: u32,
}

fn default_output() -> String {
    "both".to_string()
}

fn default_format() -> String {
    "markdown".to_string()
}

fn default_sort() -> String {
    "newest".to_string()
}

fn default_top_count() -> u32 {
    5
}

fn yes() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("tweets").required().describe(
                "Paste the entire contents of the tweets.js file from a Twitter/X data export \
                 (it lives in the archive's data/ folder and starts with \
                 `window.YTD.tweets.part0 = [`). The JavaScript wrapper is stripped \
                 automatically; a bare JSON array of tweets works too.",
            ),
        )
        .param(
            Param::enumv("output", ["transcript", "stats", "both"]).default("both").describe(
                "What to return: transcript (the tweets themselves), stats (a summary with \
                 totals, engagement, per-year activity, top hashtags/mentions/domains/apps and \
                 the most-liked tweets), or both (default).",
            ),
        )
        .param(
            Param::enumv("format", ["markdown", "text", "html", "csv"]).default("markdown").describe(
                "How to render the result: markdown (headings, summary tables and permalinks, \
                 default), text (plain readable transcript), html (escaped article blocks you \
                 can paste into a page), or csv (machine-readable rows — the transcript becomes \
                 date,id,kind,likes,retweets,language,source,text,permalink).",
            ),
        )
        .param(
            Param::enumv("sort", ["newest", "oldest", "likes", "retweets"]).default("newest").describe(
                "Transcript order: newest first (default, matching the timeline), oldest first \
                 for a chronological read, likes for the most-favourited first, or retweets for \
                 the most-reposted first.",
            ),
        )
        .param(
            Param::string("search").describe(
                "Optional: keep only tweets whose text contains this text, matched \
                 case-insensitively after t.co links are expanded (e.g. `rustlang` or \
                 `#release`). Omit to keep everything.",
            ),
        )
        .param(
            Param::string("since").describe(
                "Optional inclusive start date in YYYY-MM-DD form, compared against each \
                 tweet's UTC date (e.g. `2024-01-15`). Omit for no lower bound.",
            ),
        )
        .param(
            Param::string("until").describe(
                "Optional inclusive end date in YYYY-MM-DD form, compared against each tweet's \
                 UTC date (e.g. `2024-01-31`). Omit for no upper bound.",
            ),
        )
        .param(
            Param::boolean("include_replies").default(true).describe(
                "When true (default), tweets that reply to someone are included and labelled \
                 `reply · to @name`. Set false to keep standalone posts and retweets only.",
            ),
        )
        .param(
            Param::boolean("include_retweets").default(true).describe(
                "When true (default), retweets (a `RT @name:` post or one carrying a \
                 retweeted_status) are included. Set false to keep only what you wrote.",
            ),
        )
        .param(
            Param::boolean("expand_urls").default(true).describe(
                "When true (default), every t.co short link is rewritten to the expanded_url \
                 stored in the archive and the redundant t.co media link is dropped from the \
                 text. Set false to keep the tweet text byte-for-byte as exported.",
            ),
        )
        .param(
            Param::integer("top_count").min(0.0).max(100.0).default(5).describe(
                "How many most-liked tweets to list in the summary (default 5, 0 = skip that \
                 table). Ties break by retweets, then by date.",
            ),
        )
        .param(
            Param::integer("max_tweets").min(0.0).max(500000.0).default(0).describe(
                "Cap on how many tweets to render, applied after every filter and the sort \
                 (0 = no limit). Use it to preview a very large archive; the summary reports \
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
    name = "gizza-ai/twitter-archive-reader",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn the tweets.js file from a Twitter/X archive into a readable transcript with posting stats and top tweets",
    skill(
        description = "Read a Twitter/X data export (paste the contents of the archive's data/tweets.js — the JavaScript file that starts with `window.YTD.tweets.part0 = [`) and return a readable transcript plus posting statistics. The JS wrapper is stripped automatically, t.co short links are expanded back to the URLs stored in the archive, HTML entities are decoded, media becomes [photo: url]/[video: url] placeholders, and every tweet is classified original / reply / retweet with its UTC timestamp, likes, retweets, language, posting app and a https://twitter.com/i/web/status/<id> permalink. The summary reports totals, originals vs replies vs retweets, likes and retweets received with per-tweet averages, the date range, tweets per year, top hashtags, mentions, link domains, posting apps and languages with shares, and the most-liked tweets. Options: output (transcript/stats/both), format (markdown/text/html/csv), sort (newest/oldest/likes/retweets), search, since/until date bounds (YYYY-MM-DD, UTC), include_replies, include_retweets, expand_urls, top_count and max_tweets. Fully local and deterministic — no AI model, no network, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "twitter-archive-reader", |a: Args| {
            let opts = Options {
                output: Output::parse(&a.output),
                format: Format::parse(&a.format).map_err(SkillError::InvalidArgs)?,
                sort: Sort::parse(&a.sort).map_err(SkillError::InvalidArgs)?,
                search: a.search.filter(|s| !s.trim().is_empty()),
                since: a.since.filter(|s| !s.trim().is_empty()),
                until: a.until.filter(|s| !s.trim().is_empty()),
                include_replies: a.include_replies,
                include_retweets: a.include_retweets,
                expand_urls: a.expand_urls,
                top_count: a.top_count as usize,
                max_tweets: a.max_tweets as usize,
            };
            render(&a.tweets, &opts).map_err(SkillError::InvalidArgs)
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
                    "tweets": { "type": "string", "description": "Paste the entire contents of the tweets.js file from a Twitter/X data export (it lives in the archive's data/ folder and starts with `window.YTD.tweets.part0 = [`). The JavaScript wrapper is stripped automatically; a bare JSON array of tweets works too." },
                    "output": { "type": "string", "enum": ["transcript", "stats", "both"], "default": "both", "description": "What to return: transcript (the tweets themselves), stats (a summary with totals, engagement, per-year activity, top hashtags/mentions/domains/apps and the most-liked tweets), or both (default)." },
                    "format": { "type": "string", "enum": ["markdown", "text", "html", "csv"], "default": "markdown", "description": "How to render the result: markdown (headings, summary tables and permalinks, default), text (plain readable transcript), html (escaped article blocks you can paste into a page), or csv (machine-readable rows — the transcript becomes date,id,kind,likes,retweets,language,source,text,permalink)." },
                    "sort": { "type": "string", "enum": ["newest", "oldest", "likes", "retweets"], "default": "newest", "description": "Transcript order: newest first (default, matching the timeline), oldest first for a chronological read, likes for the most-favourited first, or retweets for the most-reposted first." },
                    "search": { "type": "string", "description": "Optional: keep only tweets whose text contains this text, matched case-insensitively after t.co links are expanded (e.g. `rustlang` or `#release`). Omit to keep everything." },
                    "since": { "type": "string", "description": "Optional inclusive start date in YYYY-MM-DD form, compared against each tweet's UTC date (e.g. `2024-01-15`). Omit for no lower bound." },
                    "until": { "type": "string", "description": "Optional inclusive end date in YYYY-MM-DD form, compared against each tweet's UTC date (e.g. `2024-01-31`). Omit for no upper bound." },
                    "include_replies": { "type": "boolean", "default": true, "description": "When true (default), tweets that reply to someone are included and labelled `reply · to @name`. Set false to keep standalone posts and retweets only." },
                    "include_retweets": { "type": "boolean", "default": true, "description": "When true (default), retweets (a `RT @name:` post or one carrying a retweeted_status) are included. Set false to keep only what you wrote." },
                    "expand_urls": { "type": "boolean", "default": true, "description": "When true (default), every t.co short link is rewritten to the expanded_url stored in the archive and the redundant t.co media link is dropped from the text. Set false to keep the tweet text byte-for-byte as exported." },
                    "top_count": { "type": "integer", "minimum": 0, "maximum": 100, "default": 5, "description": "How many most-liked tweets to list in the summary (default 5, 0 = skip that table). Ties break by retweets, then by date." },
                    "max_tweets": { "type": "integer", "minimum": 0, "maximum": 500000, "default": 0, "description": "Cap on how many tweets to render, applied after every filter and the sort (0 = no limit). Use it to preview a very large archive; the summary reports the truncation." }
                },
                "required": ["tweets"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
