//! gizza-ai/calendar-freebusy-overlap — find the free time windows common to
//! two pasted iCalendar (.ics) calendars, inside chosen working hours.
//!
//! Thin chat-skill wrapper around `gizza-ai-calendar-freebusy-overlap-core`.
//! The chat schema is single-sourced from `descriptor()` (shared with the
//! CLI); the handler delegates to `block_utils::run_skill`. The core takes an
//! explicit clock, so this wrapper supplies `SystemTime::now()` (available in
//! the wafer runtime and the CLI) for the "start_date empty = today" default.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    calendar_a: String,
    #[serde(default)]
    calendar_b: String,
    #[serde(default)]
    start_date: String,
    #[serde(default)]
    days: Option<f64>,
    #[serde(default)]
    day_start: String,
    #[serde(default)]
    day_end: String,
    #[serde(default)]
    min_minutes: Option<f64>,
    #[serde(default)]
    timezone: String,
    #[serde(default)]
    weekends: Option<bool>,
    #[serde(default)]
    output: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("calendar_a")
                .required()
                .multiline()
                .describe("The first calendar as raw iCalendar (.ics) text — paste the whole export, starting with BEGIN:VCALENDAR. Google Calendar: Settings → Import & export → Export. Outlook: Save calendar as .ics. Max 1 MiB."),
        )
        .param(
            Param::string("calendar_b")
                .required()
                .multiline()
                .describe("The second calendar as raw iCalendar (.ics) text, same format as calendar_a. Max 1 MiB."),
        )
        .param(
            Param::string("start_date")
                .describe("First day to scan, as YYYY-MM-DD (e.g. 2026-07-20). Leave empty for today in the selected timezone."),
        )
        .param(
            Param::integer("days")
                .min(1.0)
                .max(60.0)
                .default(7)
                .describe("How many consecutive days to scan, 1-60. Default 7 (one week); common presets are 7, 14, and 30."),
        )
        .param(
            Param::string("day_start")
                .default("09:00")
                .describe("Earliest slot start each day, HH:MM 24-hour clock in the selected timezone. Default 09:00."),
        )
        .param(
            Param::string("day_end")
                .default("17:00")
                .describe("Latest slot end each day, HH:MM 24-hour clock (up to 24:00 for end of day). Must be after day_start. Default 17:00."),
        )
        .param(
            Param::integer("min_minutes")
                .min(5.0)
                .max(720.0)
                .default(30)
                .describe("Only report free windows at least this many minutes long, 5-720. Default 30; typical meeting lengths are 30, 45, 60, 90, 120."),
        )
        .param(
            Param::string("timezone")
                .default("UTC")
                .describe("IANA timezone the working hours and the results are expressed in, e.g. Europe/Berlin, America/New_York, Asia/Tokyo, or UTC. Event times in the calendars are converted into this zone (DST-correct)."),
        )
        .param(
            Param::boolean("weekends")
                .default(false)
                .describe("Include Saturdays and Sundays in the scan. Default false (weekdays only)."),
        )
        .param(
            Param::enumv("output", ["text", "json", "ics"])
                .default("text")
                .describe("Result format: 'text' = readable slot list, 'json' = machine-readable slots with ISO timestamps, 'ics' = an RFC 5545 VFREEBUSY calendar of the free periods."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn now_utc_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/calendar-freebusy-overlap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare two ICS calendars and list the free time slots common to both.",
    skill(
        description = "Compare two pasted iCalendar (.ics) calendars and list the time windows where BOTH are free, inside chosen working hours. Pass each calendar's raw .ics text as 'calendar_a' / 'calendar_b' (VEVENTs incl. daily/weekly/monthly/yearly RRULEs with INTERVAL/COUNT/UNTIL/BYDAY, EXDATE, all-day events, VFREEBUSY periods; cancelled and transparent events don't block). Scan 'days' days (1-60, default 7) from 'start_date' (YYYY-MM-DD, empty = today), between 'day_start' and 'day_end' (HH:MM, default 09:00-17:00) in 'timezone' (IANA name, default UTC; event TZIDs are converted, DST-correct). 'weekends' false (default) skips Sat/Sun; 'min_minutes' (default 30) drops shorter gaps. 'output' = text (readable list), json (slots with ISO timestamps), or ics (a VFREEBUSY calendar of the free periods).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "calendar-freebusy-overlap", |a: Args| {
            gizza_ai_calendar_freebusy_overlap_core::run(
                &a.calendar_a,
                &a.calendar_b,
                &a.start_date,
                a.days.map(|d| d as i64).unwrap_or(7),
                &a.day_start,
                &a.day_end,
                a.min_minutes.map(|m| m as i64).unwrap_or(30),
                &a.timezone,
                a.weekends.unwrap_or(false),
                &a.output,
                now_utc_secs(),
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "calendar_a": { "type": "string", "description": "The first calendar as raw iCalendar (.ics) text — paste the whole export, starting with BEGIN:VCALENDAR. Google Calendar: Settings → Import & export → Export. Outlook: Save calendar as .ics. Max 1 MiB." },
                    "calendar_b": { "type": "string", "description": "The second calendar as raw iCalendar (.ics) text, same format as calendar_a. Max 1 MiB." },
                    "start_date": { "type": "string", "description": "First day to scan, as YYYY-MM-DD (e.g. 2026-07-20). Leave empty for today in the selected timezone." },
                    "days": { "type": "integer", "minimum": 1, "maximum": 60, "default": 7, "description": "How many consecutive days to scan, 1-60. Default 7 (one week); common presets are 7, 14, and 30." },
                    "day_start": { "type": "string", "default": "09:00", "description": "Earliest slot start each day, HH:MM 24-hour clock in the selected timezone. Default 09:00." },
                    "day_end": { "type": "string", "default": "17:00", "description": "Latest slot end each day, HH:MM 24-hour clock (up to 24:00 for end of day). Must be after day_start. Default 17:00." },
                    "min_minutes": { "type": "integer", "minimum": 5, "maximum": 720, "default": 30, "description": "Only report free windows at least this many minutes long, 5-720. Default 30; typical meeting lengths are 30, 45, 60, 90, 120." },
                    "timezone": { "type": "string", "default": "UTC", "description": "IANA timezone the working hours and the results are expressed in, e.g. Europe/Berlin, America/New_York, Asia/Tokyo, or UTC. Event times in the calendars are converted into this zone (DST-correct)." },
                    "weekends": { "type": "boolean", "default": false, "description": "Include Saturdays and Sundays in the scan. Default false (weekdays only)." },
                    "output": { "type": "string", "enum": ["text", "json", "ics"], "default": "text", "description": "Result format: 'text' = readable slot list, 'json' = machine-readable slots with ISO timestamps, 'ics' = an RFC 5545 VFREEBUSY calendar of the free periods." }
                },
                "required": ["calendar_a", "calendar_b"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
