//! gizza-ai/cookieless-visitor-counter — chat skill block on the shared tool abstraction.
//!
//! Counts unique visitors in a web access log using the daily-salted-hash
//! method: a visitor is `SHA-256(salt ‖ period ‖ IP ‖ user-agent)`, never a
//! cookie. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI + page); `handle()` delegates to `block_utils::run_skill`.
//! No host calls — runs entirely inside the WASM sandbox, stores nothing.
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
    identity: String,
    #[serde(default)]
    period: String,
    #[serde(default)]
    salt: String,
    #[serde(default = "default_true")]
    exclude_bots: bool,
    /// 0 → the core default (12); the core rejects anything outside 6..=64.
    #[serde(default)]
    hash_length: u32,
    #[serde(default)]
    output: String,
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
                .describe("The web access log — one request per line. Accepts Apache/nginx Combined Log Format ('1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] \"GET / HTTP/1.1\" 200 12 \"-\" \"Mozilla/5.0 ...\"'), Common Log Format, JSON/NDJSON lines (ip/remote_addr + user_agent + time keys), or CSV with a header naming an ip column. Blank lines are skipped; the limit is 200000 lines."),
        )
        .param(
            Param::enumv("format", ["auto", "combined", "common", "json", "csv"])
                .default("auto")
                .describe("How to read each line. 'auto' (default) sniffs the format from the first non-blank line. 'combined' = Apache/nginx Combined Log Format (user-agent is the last quoted field). 'common' = Common Log Format, which has no user-agent field at all. 'json' = one JSON object per line, keys matched case-insensitively from ip/remote_addr/client_ip, user_agent/ua/http_user_agent, and time/timestamp/time_local. 'csv' = comma-separated with a header row naming those same columns."),
        )
        .param(
            Param::enumv("identity", ["ip_ua", "ip", "network_ua"])
                .default("ip_ua")
                .describe("What identifies a visitor before hashing. 'ip_ua' (default) is IP + user-agent, the convention used by privacy-first analytics and GoAccess. 'ip' is the IP alone, the older AWStats convention — coarser, and merges every browser on one machine into one visitor. 'network_ua' truncates the address to its network first (IPv4 to /24, IPv6 to /48) then adds the user-agent, matching Matomo/Google-Analytics style IP anonymisation. The raw value is only ever hashed, never returned."),
        )
        .param(
            Param::enumv("period", ["hour", "day", "month", "total"])
                .default("day")
                .describe("The salt-rotation and bucketing window. 'day' (default) is the classic daily-salted-hash: one row per calendar date, and a visitor's ID changes at midnight so it cannot be linked across days. 'hour' and 'month' rotate and bucket by hour ('2026-08-06 10:00') or month ('2026-08'). 'total' puts the whole log in one bucket labelled 'all'. Because IDs are un-linkable across periods, per-period uniques do NOT sum — the report also gives the distinct-visitor count over the whole log."),
        )
        .param(
            Param::string("salt")
                .default("")
                .describe("Secret salt mixed into every hash. Leave blank (default) to use a fixed built-in salt, which makes runs reproducible; set your own secret string to make the visitor IDs unguessable and specific to you. Changing the salt changes every ID, so IDs from different salts can never be correlated. The salt is never included in the output."),
        )
        .param(
            Param::boolean("exclude_bots")
                .default(true)
                .describe("Skip crawler and script hits before counting. Default true — bots would otherwise inflate the visitor count. Matches the user-agent case-insensitively against known crawlers (search engines, AI crawlers, SEO and monitoring tools, HTTP libraries, headless browsers) plus the bot/crawl/spider/slurp token heuristic; a missing or '-' user-agent also counts as a bot. Detection is by declared user-agent only — no reverse DNS or IP-range verification. Set false to count every request."),
        )
        .param(
            Param::integer("hash_length")
                .default(gizza_ai_cookieless_visitor_counter_core::DEFAULT_HASH_LENGTH as i64)
                .min(gizza_ai_cookieless_visitor_counter_core::MIN_HASH_LENGTH as f64)
                .max(gizza_ai_cookieless_visitor_counter_core::MAX_HASH_LENGTH as f64)
                .describe("How many hex characters of the SHA-256 digest form each visitor ID (6-64, default 12). Only affects the ids output and the chance of two visitors colliding; 12 is ample for a single site's daily traffic. Counting itself always uses the truncated ID, so a very short length can merge distinct visitors."),
        )
        .param(
            Param::enumv("output", ["report", "table", "json", "csv", "ids"])
                .default("report")
                .describe("What to return. 'report' (default) is a readable summary: the method and settings, a per-period table of visitors/pageviews/views-per-visitor, and totals including the distinct-visitor count. 'table' is that table as Markdown; 'csv' is the same rows as CSV; 'json' is a structured object with periods plus totals. 'ids' lists the pseudonymous visitor ID for each parsed request (capped at 5000 rows) so you can verify no IP or user-agent survives."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CookielessVisitorCounter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cookieless-visitor-counter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Count unique visitors in an access log with the daily-salted-hash method — no cookies, no PII stored.",
    skill(
        description = "Count unique visitors in a web access log using the daily-salted-hash method used by privacy-first analytics — no cookies, no tracking, and no IP or user-agent retained. Each request is reduced to SHA-256(salt ‖ period ‖ identity); because the period key is inside the hash, a visitor's ID is different every day and cannot be linked across days, the same guarantee a server gets by rotating its salt every 24 hours. Reads Apache/nginx Combined and Common log formats, JSON/NDJSON lines, and CSV (format='auto' sniffs). identity picks what is hashed: 'ip_ua' (default, IP + user-agent, the Plausible/GoAccess convention), 'ip' (the AWStats convention), or 'network_ua' (IPv4 truncated to /24 and IPv6 to /48 first, Matomo/GA-style). period sets the rotation and bucket window: hour, day (default), month, or total. Set salt to your own secret to make IDs unguessable. exclude_bots (default true) drops crawler and script hits by user-agent. output='report' (default) summarises per-period visitors, pageviews and views-per-visitor plus totals; 'table'/'csv'/'json' give the same rows as data; 'ids' shows the pseudonymous ID per request. Per-period uniques do not sum, so the totals also report distinct visitors over the whole log. Runs locally: logs cannot separate people behind a shared NAT, and a changing IP or user-agent splits one person into several.",
        parameters = schema_json()
    ),
)]
impl CookielessVisitorCounter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "cookieless-visitor-counter", |a: Args| {
            gizza_ai_cookieless_visitor_counter_core::count(
                &a.input,
                &a.format,
                &a.identity,
                &a.period,
                &a.salt,
                a.exclude_bots,
                a.hash_length,
                &a.output,
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
                    "input": { "type": "string", "description": "The web access log — one request per line. Accepts Apache/nginx Combined Log Format ('1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] \"GET / HTTP/1.1\" 200 12 \"-\" \"Mozilla/5.0 ...\"'), Common Log Format, JSON/NDJSON lines (ip/remote_addr + user_agent + time keys), or CSV with a header naming an ip column. Blank lines are skipped; the limit is 200000 lines." },
                    "format": { "type": "string", "enum": ["auto", "combined", "common", "json", "csv"], "default": "auto", "description": "How to read each line. 'auto' (default) sniffs the format from the first non-blank line. 'combined' = Apache/nginx Combined Log Format (user-agent is the last quoted field). 'common' = Common Log Format, which has no user-agent field at all. 'json' = one JSON object per line, keys matched case-insensitively from ip/remote_addr/client_ip, user_agent/ua/http_user_agent, and time/timestamp/time_local. 'csv' = comma-separated with a header row naming those same columns." },
                    "identity": { "type": "string", "enum": ["ip_ua", "ip", "network_ua"], "default": "ip_ua", "description": "What identifies a visitor before hashing. 'ip_ua' (default) is IP + user-agent, the convention used by privacy-first analytics and GoAccess. 'ip' is the IP alone, the older AWStats convention — coarser, and merges every browser on one machine into one visitor. 'network_ua' truncates the address to its network first (IPv4 to /24, IPv6 to /48) then adds the user-agent, matching Matomo/Google-Analytics style IP anonymisation. The raw value is only ever hashed, never returned." },
                    "period": { "type": "string", "enum": ["hour", "day", "month", "total"], "default": "day", "description": "The salt-rotation and bucketing window. 'day' (default) is the classic daily-salted-hash: one row per calendar date, and a visitor's ID changes at midnight so it cannot be linked across days. 'hour' and 'month' rotate and bucket by hour ('2026-08-06 10:00') or month ('2026-08'). 'total' puts the whole log in one bucket labelled 'all'. Because IDs are un-linkable across periods, per-period uniques do NOT sum — the report also gives the distinct-visitor count over the whole log." },
                    "salt": { "type": "string", "default": "", "description": "Secret salt mixed into every hash. Leave blank (default) to use a fixed built-in salt, which makes runs reproducible; set your own secret string to make the visitor IDs unguessable and specific to you. Changing the salt changes every ID, so IDs from different salts can never be correlated. The salt is never included in the output." },
                    "exclude_bots": { "type": "boolean", "default": true, "description": "Skip crawler and script hits before counting. Default true — bots would otherwise inflate the visitor count. Matches the user-agent case-insensitively against known crawlers (search engines, AI crawlers, SEO and monitoring tools, HTTP libraries, headless browsers) plus the bot/crawl/spider/slurp token heuristic; a missing or '-' user-agent also counts as a bot. Detection is by declared user-agent only — no reverse DNS or IP-range verification. Set false to count every request." },
                    "hash_length": { "type": "integer", "minimum": 6, "maximum": 64, "default": 12, "description": "How many hex characters of the SHA-256 digest form each visitor ID (6-64, default 12). Only affects the ids output and the chance of two visitors colliding; 12 is ample for a single site's daily traffic. Counting itself always uses the truncated ID, so a very short length can merge distinct visitors." },
                    "output": { "type": "string", "enum": ["report", "table", "json", "csv", "ids"], "default": "report", "description": "What to return. 'report' (default) is a readable summary: the method and settings, a per-period table of visitors/pageviews/views-per-visitor, and totals including the distinct-visitor count. 'table' is that table as Markdown; 'csv' is the same rows as CSV; 'json' is a structured object with periods plus totals. 'ids' lists the pseudonymous visitor ID for each parsed request (capped at 5000 rows) so you can verify no IP or user-agent survives." }
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
