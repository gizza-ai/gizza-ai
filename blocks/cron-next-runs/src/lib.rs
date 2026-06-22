//! gizza-ai/cron-next-runs — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute — the only
//! host capability used is the clock (`SystemTime::now`) to resolve "now" when
//! the caller does not pass an explicit base time.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    expression: String,
    #[serde(default)]
    after: String,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("expression").required().describe(
                "The cron expression to evaluate. Standard 5-field Vixie/crontab syntax \
'minute hour day-of-month month day-of-week' (e.g. '*/15 * * * *', '0 9 * * MON-FRI', \
'0 0 1 * *'), or 6 fields with a leading seconds field (e.g. '*/30 * * * * *'). Each \
field supports '*', single values, 'A-B' ranges, 'A-B/S' and '*/S' steps, comma lists \
'A,B,C', three-letter month names (JAN-DEC) and weekday names (SUN-SAT); day-of-week \
accepts both 0 and 7 for Sunday. Shortcuts @yearly, @annually, @monthly, @weekly, \
@daily, @midnight and @hourly are also accepted. All times are UTC.",
            ),
        )
        .param(
            Param::string("after").describe(
                "Optional base time; the next runs are computed strictly AFTER this instant. \
Accepts an ISO-8601 / RFC-3339 UTC timestamp (e.g. '2025-01-01T09:00:00Z', \
'2025-01-01 09:00', or a bare date '2025-01-01'), or a Unix epoch second. Defaults to \
the current time (UTC) when omitted.",
            ),
        )
        .param(
            Param::integer("count")
                .min(1.0)
                .max(100.0)
                .describe("How many upcoming run times to return (1-100, default 5)."),
        )
        .param(
            Param::enumv("format", ["text", "json"]).default("text").describe(
                "Output format. 'text' (default) is an aligned human-readable list of the next \
run times (UTC, with weekday) plus a plain-English description of the schedule. 'json' is a \
machine-readable object with the expression, a plain-English description, the base time, and \
an array of { iso, epoch, weekday } entries.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Resolve the base time: parse `after` if given, else use the current clock.
/// On wasm (chat block + CLI runtime) the host provides the clock import.
fn resolve_base(after: &str) -> Result<i64, String> {
    let after = after.trim();
    if !after.is_empty() {
        return gizza_ai_cron_next_runs_core::parse_timestamp(after);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_string())?;
    Ok(now.as_secs() as i64)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cron-next-runs",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the next N scheduled run times for a cron expression.",
    skill(
        description = "Compute the next upcoming run times for a cron schedule. Set 'expression' \
to a standard 5-field crontab string ('minute hour day-of-month month day-of-week', e.g. \
'*/15 * * * *', '0 9 * * MON-FRI', '0 0 1 * *'), a 6-field form with a leading seconds field \
('*/30 * * * * *'), or a shortcut (@hourly, @daily, @weekly, @monthly, @yearly). Fields support \
ranges (A-B), steps (*/S, A-B/S), lists (A,B,C), and three-letter month/weekday names; \
day-of-week accepts 0 and 7 for Sunday, and a restricted day-of-month plus day-of-week match by \
union (Vixie behavior). It returns the next 'count' run times (default 5) strictly AFTER 'after' \
(an ISO-8601/RFC-3339 UTC timestamp, a Unix epoch second, or — when omitted — now). All times \
are UTC. Choose 'format' = 'text' (aligned report) or 'json' (machine-readable). Use this to \
preview when a cron job will fire or to validate a crontab line.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cron-next-runs", |a: Args| {
            let base = resolve_base(&a.after).map_err(SkillError::InvalidArgs)?;
            let count = a.count.unwrap_or(5);
            let fmt = if a.format.trim().is_empty() {
                "text"
            } else {
                a.format.trim()
            };
            gizza_ai_cron_next_runs_core::next_runs(&a.expression, base, count, fmt)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The cron expression to evaluate. Standard 5-field Vixie/crontab syntax 'minute hour day-of-month month day-of-week' (e.g. '*/15 * * * *', '0 9 * * MON-FRI', '0 0 1 * *'), or 6 fields with a leading seconds field (e.g. '*/30 * * * * *'). Each field supports '*', single values, 'A-B' ranges, 'A-B/S' and '*/S' steps, comma lists 'A,B,C', three-letter month names (JAN-DEC) and weekday names (SUN-SAT); day-of-week accepts both 0 and 7 for Sunday. Shortcuts @yearly, @annually, @monthly, @weekly, @daily, @midnight and @hourly are also accepted. All times are UTC."
                    },
                    "after": {
                        "type": "string",
                        "description": "Optional base time; the next runs are computed strictly AFTER this instant. Accepts an ISO-8601 / RFC-3339 UTC timestamp (e.g. '2025-01-01T09:00:00Z', '2025-01-01 09:00', or a bare date '2025-01-01'), or a Unix epoch second. Defaults to the current time (UTC) when omitted."
                    },
                    "count": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "description": "How many upcoming run times to return (1-100, default 5)."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json"],
                        "default": "text",
                        "description": "Output format. 'text' (default) is an aligned human-readable list of the next run times (UTC, with weekday) plus a plain-English description of the schedule. 'json' is a machine-readable object with the expression, a plain-English description, the base time, and an array of { iso, epoch, weekday } entries."
                    }
                },
                "required": ["expression"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
