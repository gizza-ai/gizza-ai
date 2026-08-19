//! gizza-ai/youtube-takeout-stats — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_report")]
    report: String,
    #[serde(default = "default_top")]
    top: f64,
    #[serde(default)]
    utc_offset: f64,
    #[serde(default)]
    include_ads: bool,
    #[serde(default)]
    include_music: bool,
    #[serde(default)]
    start_date: String,
    #[serde(default)]
    end_date: String,
}

fn default_output() -> String {
    "text".to_string()
}
fn default_report() -> String {
    "overview".to_string()
}
fn default_top() -> f64 {
    10.0
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Contents of a Google/YouTube Takeout watch-history.json or watch-history.html export. The tool runs locally and never fetches YouTube or uploads the history."),
        )
        .param(
            Param::enumv("output", ["text", "csv", "json"])
                .default("text")
                .describe("Output format: full text dashboard, CSV table for the selected report, or structured JSON."),
        )
        .param(
            Param::enumv("report", ["overview", "channels", "videos", "months", "weekdays", "hours"])
                .default("overview")
                .describe("Report section to emit. Overview includes headline totals plus the channel, video, month, weekday, and hour tables."),
        )
        .param(
            Param::integer("top")
                .min(1.0)
                .max(100.0)
                .default(10)
                .describe("Maximum ranked rows to include in the top channels and top videos reports, from 1 to 100."),
        )
        .param(
            Param::number("utc_offset")
                .min(-14.0)
                .max(14.0)
                .default(0.0)
                .describe("Hours to add to JSON export timestamps before bucketing days and hours. Use your local UTC offset, such as -5 or 5.5. HTML exports are already local and ignore this value."),
        )
        .param(
            Param::boolean("include_ads")
                .default(false)
                .describe("Include Google Ads impressions from the export. Off by default so ad rows do not inflate viewing totals."),
        )
        .param(
            Param::boolean("include_music")
                .default(false)
                .describe("Include YouTube Music watch rows. Off by default to keep the report focused on regular YouTube watch history."),
        )
        .param(
            Param::string("start_date")
                .default("")
                .describe("Optional inclusive start date in YYYY-MM-DD format. Leave blank to start at the first watch in the export."),
        )
        .param(
            Param::string("end_date")
                .default("")
                .describe("Optional inclusive end date in YYYY-MM-DD format. Leave blank to end at the last watch in the export."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/youtube-takeout-stats",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize Google/YouTube Takeout watch history into viewing stats.",
    skill(
        description = "Parse a Google/YouTube Takeout watch-history.json or watch-history.html export into local viewing statistics: totals, top channels, repeated videos, month trends, weekday and hour patterns, streaks, filters, CSV, and JSON. No YouTube API key, account login, or network lookup is used.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "youtube-takeout-stats", |a: Args| {
            gizza_ai_youtube_takeout_stats_core::run(
                &a.input,
                &a.output,
                &a.report,
                a.top,
                a.utc_offset,
                a.include_ads,
                a.include_music,
                &a.start_date,
                &a.end_date,
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert_eq!(schema.get("type").unwrap(), "object");
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
        assert_eq!(
            schema.get("required").unwrap(),
            &serde_json::json!(["input"])
        );
        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["text", "csv", "json"])
        );
        assert_eq!(
            props["report"]["enum"],
            serde_json::json!(["overview", "channels", "videos", "months", "weekdays", "hours"])
        );
        assert_eq!(props["top"]["default"], 10);
        assert_eq!(props["utc_offset"]["minimum"], -14.0);
        assert_eq!(props["utc_offset"]["maximum"], 14.0);
        assert_eq!(props["include_ads"]["default"], false);
        assert_eq!(props["include_music"]["default"], false);
        for key in [
            "input",
            "output",
            "report",
            "top",
            "utc_offset",
            "include_ads",
            "include_music",
            "start_date",
            "end_date",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing .describe() for {key}"
            );
        }
    }
}
