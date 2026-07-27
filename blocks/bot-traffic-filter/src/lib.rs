//! gizza-ai/bot-traffic-filter — chat skill block on the shared tool abstraction.
//!
//! Classifies each entry of an access log or event list as bot/crawler vs human
//! by its user-agent, strips the bot hits, and reports the human-versus-bot
//! split. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI + page); `handle()` delegates to `block_utils::run_skill`. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_true")]
    empty_is_bot: bool,
    /// 0 → the core default (500); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The access log or event list — one hit per line. Paste Apache/nginx Combined Log Format lines (the user-agent is the last quoted field) or a bare list of user-agent strings, one per line. Blank lines are skipped."),
        )
        .param(
            Param::enumv("format", ["auto", "combined", "plain"])
                .default("auto")
                .describe("How to read each line. 'auto' (default) uses the last quoted field as the user-agent when the line is an access-log entry, otherwise treats the whole line as a user-agent. 'combined' forces Apache/nginx Combined Log Format (UA = last quoted field). 'plain' treats every line as a bare user-agent string."),
        )
        .param(
            Param::enumv("output", ["report", "table", "json", "csv", "humans", "bots"])
                .default("report")
                .describe("What to return. 'report' (default) is a summary: totals, the human/bot split with percentages, a per-category breakdown, and the top bots. 'table' is a Markdown table (one row per hit). 'json'/'csv' are per-hit data. 'humans' returns only the original human lines (bots stripped); 'bots' returns only the original bot lines."),
        )
        .param(
            Param::boolean("empty_is_bot")
                .default(true)
                .describe("Treat a missing or '-' user-agent as a bot. Default true — scripts and scrapers often send no user-agent. Set false to count blank-UA hits as human."),
        )
        .param(
            Param::integer("limit")
                .default(500)
                .min(1.0)
                .max(gizza_ai_bot_traffic_filter_core::MAX_LIMIT as f64)
                .describe("Maximum number of rows to output for the table/json/csv/humans/bots outputs (1-10000). Default 500. The 'report' summary always counts every line regardless of this cap."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BotTrafficFilter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bot-traffic-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag and strip bot/crawler hits from a log by user-agent and report the human-vs-bot split.",
    skill(
        description = "Classify each entry of an access log or event list as bot/crawler vs human by its user-agent, strip the bot hits, and report the human-versus-bot split. Detection matches the user-agent (case-insensitively) against a curated list of known crawlers/agents — search engines (Googlebot, Bingbot), AI crawlers (GPTBot, ClaudeBot, PerplexityBot), SEO tools (AhrefsBot, SemrushBot), monitoring probes, social/link-preview fetchers, generic HTTP libraries (curl, python-requests, Go-http-client), and headless browsers — plus the standard bot/crawl/spider/slurp token heuristic; a missing/'-' user-agent counts as a bot when empty_is_bot is true (default). format='auto' (default) pulls the UA from the last quoted field of an access-log line or treats the whole line as a UA. output='report' (default) summarizes the split and categories; 'table'/'json'/'csv' give per-hit rows; 'humans' returns the human lines only (bots stripped) and 'bots' the bot lines only. limit caps the row count (default 500). Runs locally: no DNS/IP-range/behavioural checks, so a spoofed user-agent is classified by what it declares.",
        parameters = schema_json()
    ),
)]
impl BotTrafficFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "bot-traffic-filter", |a: Args| {
            gizza_ai_bot_traffic_filter_core::filter(
                &a.input,
                &a.format,
                &a.output,
                a.empty_is_bot,
                a.limit,
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
                    "input": { "type": "string", "description": "The access log or event list — one hit per line. Paste Apache/nginx Combined Log Format lines (the user-agent is the last quoted field) or a bare list of user-agent strings, one per line. Blank lines are skipped." },
                    "format": { "type": "string", "enum": ["auto", "combined", "plain"], "default": "auto", "description": "How to read each line. 'auto' (default) uses the last quoted field as the user-agent when the line is an access-log entry, otherwise treats the whole line as a user-agent. 'combined' forces Apache/nginx Combined Log Format (UA = last quoted field). 'plain' treats every line as a bare user-agent string." },
                    "output": { "type": "string", "enum": ["report", "table", "json", "csv", "humans", "bots"], "default": "report", "description": "What to return. 'report' (default) is a summary: totals, the human/bot split with percentages, a per-category breakdown, and the top bots. 'table' is a Markdown table (one row per hit). 'json'/'csv' are per-hit data. 'humans' returns only the original human lines (bots stripped); 'bots' returns only the original bot lines." },
                    "empty_is_bot": { "type": "boolean", "default": true, "description": "Treat a missing or '-' user-agent as a bot. Default true — scripts and scrapers often send no user-agent. Set false to count blank-UA hits as human." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 500, "description": "Maximum number of rows to output for the table/json/csv/humans/bots outputs (1-10000). Default 500. The 'report' summary always counts every line regardless of this cap." }
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
