//! gizza-ai/ics-agenda-view — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Builds a deterministic
//! day-by-day agenda from pasted iCalendar text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ics: String,
    #[serde(default)]
    start_date: String,
    #[serde(default = "default_days")]
    days: i64,
    #[serde(default = "default_timezone")]
    timezone: String,
    #[serde(default = "default_day_start")]
    day_start: String,
    #[serde(default = "default_day_end")]
    day_end: String,
    #[serde(default = "default_min_gap_minutes")]
    min_gap_minutes: i64,
    #[serde(default = "default_true")]
    show_gaps: bool,
    #[serde(default)]
    filter: String,
    #[serde(default = "default_true")]
    expand_recurring: bool,
    #[serde(default)]
    include_cancelled: bool,
    #[serde(default = "default_details")]
    details: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_days() -> i64 {
    7
}
fn default_timezone() -> String {
    "UTC".to_string()
}
fn default_day_start() -> String {
    "09:00".to_string()
}
fn default_day_end() -> String {
    "18:00".to_string()
}
fn default_min_gap_minutes() -> i64 {
    30
}
fn default_true() -> bool {
    true
}
fn default_details() -> String {
    "normal".to_string()
}
fn default_output() -> String {
    "text".to_string()
}

/// Single source for the chat schema (and CLI + page manifest).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("ics").required().describe("The full iCalendar text to parse, including BEGIN:VCALENDAR and one or more VEVENT entries. Supports LF or CRLF line endings, folded lines, TEXT escaping, DTSTART/DTEND/DURATION, all-day events, TZID parameters, EXDATE, and a bounded RRULE subset. Up to 1 MiB."))
        .param(Param::string("start_date").default("").describe("Optional agenda start date as YYYY-MM-DD in the display timezone. Leave blank to start at the earliest event date after recurrence expansion."))
        .param(Param::integer("days").default(7).min(1.0).max(90.0).describe("Number of days to render, from 1 to 90. Default 7."))
        .param(Param::string("timezone").default("UTC").describe("Display timezone as an IANA name such as UTC, Europe/Berlin, or America/New_York. Floating and all-day event times are interpreted in this zone; UTC and recognized TZID event times are converted into it."))
        .param(Param::string("day_start").default("09:00").describe("Start of the daily free-gap search window in 24-hour HH:MM time. Default 09:00."))
        .param(Param::string("day_end").default("18:00").describe("End of the daily free-gap search window in 24-hour HH:MM time. Must be after day_start; 24:00 is accepted. Default 18:00."))
        .param(Param::integer("min_gap_minutes").default(30).min(5.0).max(480.0).describe("Only show free gaps at least this many minutes long, from 5 to 480. Default 30."))
        .param(Param::boolean("show_gaps").default(true).describe("Show free gaps between meetings inside the day_start/day_end window and include empty days. Turn off for a compact agenda with events only. Default true."))
        .param(Param::string("filter").default("").describe("Optional case-insensitive text filter. Matches event summary, location, description, organizer, UID, and status before the agenda is rendered. Leave blank for all events."))
        .param(Param::boolean("expand_recurring").default(true).describe("Expand supported RRULEs (daily, weekly, monthly, yearly with interval/count/until/byday/bymonthday) inside the selected window. Turn off to list each recurring series only once. Default true."))
        .param(Param::boolean("include_cancelled").default(false).describe("Include VEVENT entries whose STATUS is CANCELLED. Default false."))
        .param(Param::enumv("details", ["compact", "normal", "full"]).default("normal").describe("How much event detail to show. compact prints times and summaries; normal adds location and recurrence/cancelled markers; full also includes organizer, UID, status, and a truncated description."))
        .param(Param::enumv("output", ["text", "markdown", "json"]).default("text").describe("Output format. text is an aligned agenda, markdown is a notes-friendly heading/list format, and json returns days, events, gaps, totals, and warnings."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ics-agenda-view",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn an iCalendar .ics file into a day-by-day agenda with free-gap detection.",
    skill(
        description = "Parse pasted iCalendar (.ics) text into a deterministic agenda grouped by day. Converts UTC, floating, all-day and recognized TZID event times into a chosen display timezone; unfolds folded lines; unescapes text fields; expands a bounded RRULE subset with EXDATE; hides cancelled events by default; and finds free gaps inside a configurable daily working window. Output as plain text, Markdown or JSON. Runs locally with no calendar account, URL fetch or network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ics-agenda-view", |a: Args| {
            gizza_ai_ics_agenda_view_core::run(
                &a.ics,
                &a.start_date,
                a.days,
                &a.timezone,
                &a.day_start,
                &a.day_end,
                a.min_gap_minutes,
                a.show_gaps,
                &a.filter,
                a.expand_recurring,
                a.include_cancelled,
                &a.details,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived.get("properties").unwrap();
        assert!(props
            .get("ics")
            .unwrap()
            .get("description")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("BEGIN:VCALENDAR"));
        assert_eq!(
            props
                .get("output")
                .unwrap()
                .get("enum")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            props.get("details").unwrap().get("default").unwrap(),
            "normal"
        );
        assert_eq!(
            derived.get("required").unwrap().as_array().unwrap()[0],
            "ics"
        );
    }
}
