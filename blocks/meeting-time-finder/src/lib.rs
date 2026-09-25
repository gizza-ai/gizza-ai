//! gizza-ai/meeting-time-finder — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. All real work lives in
//! the core crate so chat, CLI and the browser page share one implementation.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    participants: String,
    date: String,
    #[serde(default = "default_duration")]
    duration_minutes: f64,
    #[serde(default = "default_granularity")]
    granularity_minutes: String,
    #[serde(default = "default_work_start")]
    work_start: String,
    #[serde(default = "default_work_end")]
    work_end: String,
    #[serde(default = "default_max_results")]
    max_results: f64,
    #[serde(default = "default_clock")]
    clock: String,
    #[serde(default)]
    display_zone: String,
    #[serde(default = "default_true")]
    skip_weekends: bool,
    #[serde(default = "default_true")]
    allow_partial: bool,
    #[serde(default = "default_format")]
    output_format: String,
}

fn default_duration() -> f64 {
    60.0
}
fn default_granularity() -> String {
    "60".to_string()
}
fn default_work_start() -> String {
    "09:00".to_string()
}
fn default_work_end() -> String {
    "17:00".to_string()
}
fn default_max_results() -> f64 {
    5.0
}
fn default_clock() -> String {
    "24h".to_string()
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "summary".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("participants")
                .required()
                .describe(
                    "Comma-separated participants, up to 12. Each entry is an IANA timezone, \
                     optionally prefixed with a name and suffixed with that person's local working \
                     hours: 'Name@Zone:start-end'. Examples: 'Europe/London, America/New_York' or \
                     'Alice@Europe/London, Bob@Asia/Tokyo:10-18'. Entries without hours use \
                     work_start/work_end. Overnight windows such as '22:00-06:00' are supported.",
                ),
        )
        .param(
            Param::string("date")
                .required()
                .describe(
                    "The calendar date to search, as YYYY-MM-DD, read in the display zone. \
                     Example: 2026-10-01. There is no clock in this tool, so the date is always \
                     explicit and the same inputs always give the same answer.",
                ),
        )
        .param(
            Param::number("duration_minutes")
                .default(60.0)
                .min(5.0)
                .max(480.0)
                .describe("Meeting length in minutes, from 5 to 480. Default 60."),
        )
        .param(
            Param::enumv("granularity_minutes", ["15", "30", "60"])
                .default("60")
                .describe(
                    "Step between candidate start times. 60 checks each hour, 30 each half hour, \
                     15 each quarter hour. Default 60.",
                ),
        )
        .param(
            Param::string("work_start")
                .default("09:00")
                .describe(
                    "Default local working-day start for participants without their own hours. \
                     Accepts '09:00', '9', '9am' or '08:30'. Default 09:00.",
                ),
        )
        .param(
            Param::string("work_end")
                .default("17:00")
                .describe(
                    "Default local working-day end for participants without their own hours. \
                     Accepts '17:00', '17', '5pm' or '16:30'. Default 17:00.",
                ),
        )
        .param(
            Param::integer("max_results")
                .default(5.0)
                .min(1.0)
                .max(24.0)
                .describe("How many ranked slots to return, from 1 to 24. Default 5."),
        )
        .param(
            Param::enumv("clock", ["24h", "12h"])
                .default("24h")
                .describe("Clock style for every rendered time. Default 24h."),
        )
        .param(
            Param::string("display_zone")
                .default("")
                .describe(
                    "IANA timezone the ranked slot times are shown in, e.g. 'UTC' or \
                     'America/Chicago'. Leave empty to use the first participant's zone.",
                ),
        )
        .param(
            Param::boolean("skip_weekends")
                .default(true)
                .describe(
                    "Treat a participant's local Saturday or Sunday as non-working, so weekend \
                     slots score 0 for them. Turn off for teams that meet at weekends. Default true.",
                ),
        )
        .param(
            Param::boolean("allow_partial")
                .default(true)
                .describe(
                    "When no slot suits everyone, still rank the closest compromises and name who \
                     is outside their hours. Turn off to get an explicit 'no slot works for \
                     everyone' answer instead. Default true.",
                ),
        )
        .param(
            Param::enumv(
                "output_format",
                ["summary", "timeline", "table", "json", "csv", "ics"],
            )
            .default("summary")
            .describe(
                "Output shape. summary ranks slots with a per-person breakdown; timeline draws a \
                 24-hour overlap grid; table is one row per slot; json and csv are machine-readable; \
                 ics emits a calendar event for the top-ranked slot.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, SkillError> {
    gizza_ai_meeting_time_finder_core::find(
        &a.participants,
        &a.date,
        &a.work_start,
        &a.work_end,
        a.duration_minutes,
        &a.granularity_minutes,
        a.max_results,
        &a.clock,
        &a.display_zone,
        a.skip_weekends,
        a.allow_partial,
        &a.output_format,
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct MeetingTimeFinder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/meeting-time-finder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find the meeting slots on a date that fall inside every participant's working hours, ranked fairness-first.",
    skill(
        description = "Meeting time finder across timezones. Give participants as a comma-separated list of IANA zones, optionally as 'Name@Zone:start-end' (e.g. 'Alice@Europe/London, Bob@Asia/Tokyo:10-18', up to 12 people), plus the date to search. It enumerates candidate starts across the day, scores every participant on how much of the meeting lands inside their local working hours and how close it sits to the middle of their day, then ranks slots so the worst-off person is as well off as possible. Options cover meeting duration, 15/30/60-minute granularity, default working hours, 12/24-hour clock, display timezone, weekend skipping, partial-overlap fallback, and summary, timeline, table, JSON, CSV or .ics output. Daylight-saving transitions on the date are flagged. Pure date arithmetic over the IANA database, no network and no clock.",
        parameters = schema_json()
    ),
)]
impl MeetingTimeFinder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "meeting-time-finder", run_args) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_args_defaults_find_the_london_new_york_overlap() {
        let a: Args = serde_json::from_str(
            r#"{"participants":"Alice@Europe/London, Bob@America/New_York","date":"2026-10-01"}"#,
        )
        .unwrap();
        let out = run_args(a).unwrap();
        assert!(out.contains("works for all 2"), "{out}");
        assert!(out.contains("slots shown in Europe/London"), "{out}");
    }

    #[test]
    fn run_args_honours_output_format_and_display_zone() {
        let a: Args = serde_json::from_str(
            r#"{"participants":"Europe/London","date":"2026-10-01","display_zone":"UTC","output_format":"json","max_results":1}"#,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&run_args(a).unwrap()).unwrap();
        assert_eq!(v["display_zone"], "UTC");
        assert_eq!(v["slots"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn run_args_rejects_an_unknown_zone() {
        let a: Args =
            serde_json::from_str(r#"{"participants":"Mars/Olympus","date":"2026-10-01"}"#).unwrap();
        let err = format!("{:?}", run_args(a).unwrap_err());
        assert!(err.contains("unknown timezone"), "{err}");
    }

    #[test]
    fn descriptor_schema_contains_required_fields_and_enums() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            schema["required"],
            serde_json::json!(["participants", "date"])
        );
        assert_eq!(schema["properties"]["output_format"]["enum"][0], "summary");
        assert_eq!(schema["properties"]["granularity_minutes"]["enum"][2], "60");
        assert_eq!(schema["properties"]["skip_weekends"]["type"], "boolean");
    }
}
