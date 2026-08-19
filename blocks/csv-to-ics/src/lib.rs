//! gizza-ai/csv-to-ics — chat skill block on the shared tool abstraction.
//! Turns a CSV event list with a header row into one iCalendar (.ics) document.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    csv: String,
    #[serde(default = "default_timezone")]
    timezone: String,
    #[serde(default = "default_duration")]
    default_duration_minutes: f64,
    #[serde(default)]
    include_alarm: bool,
}

fn default_timezone() -> String {
    "floating".to_string()
}
fn default_duration() -> f64 {
    60.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("csv")
                .required()
                .describe("The event list as CSV text, first row naming the columns. Quoted cells (\"Lunch, with team\") are read as one value. Columns are matched case- and punctuation-insensitively: title/summary/name/event and start/start_date/begins/date are required, and end/end_date/ends, duration_minutes/duration, description/details/notes, location/place, uid/id and all_day/all-day are optional. Dates are 2026-07-24, 2026-07-24 09:00 or 2026-07-24T09:00, each with optional seconds; a start with no time makes an all-day event. Up to 5000 rows per run."),
        )
        .param(
            Param::enumv("timezone", ["floating", "utc"])
                .default("floating")
                .describe("How the times in the CSV are anchored. \"floating\" (default) writes them with no zone marker, so every calendar shows 09:00 as 09:00 in whatever timezone the reader is in — right for a schedule everyone attends locally. \"utc\" treats the pasted times as already being UTC and writes them with the trailing Z, so clients convert them to each person's own zone. All-day rows are unaffected either way."),
        )
        .param(
            Param::integer("default_duration_minutes")
                .default(60)
                .min(1.0)
                .max(1440.0)
                .describe("How long a timed event lasts, in minutes, when its row gives neither an end nor a duration_minutes cell. 60 by default, 1–1440 (one day). A row's own end or duration_minutes column always wins over this, and all-day rows ignore it."),
        )
        .param(
            Param::boolean("include_alarm")
                .default(false)
                .describe("Add a pop-up reminder 15 minutes before every event (a VALARM with ACTION:DISPLAY and the event title as its message). Off by default. For an all-day event this fires 15 minutes before midnight on the day it starts, so leave it off for holiday or deadline calendars."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvToIcs;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-ics",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a CSV event list into an importable iCalendar (.ics) calendar file",
    skill(
        description = "Turn a CSV event list into an iCalendar (.ics) file that Google Calendar, Outlook, Apple Calendar and every other calendar app can import. Paste the CSV into `csv` with a header row; column names are matched case- and punctuation-insensitively, so `Start Date`, `start_date` and `START-DATE` are the same column. A title column (title, summary, name or event) and a start column (start, start_date, begins or date) are required; end/end_date/ends, duration_minutes/duration, description/details/notes, location/place, uid/id and all_day/all-day are optional. Dates are 2026-07-24, 2026-07-24 09:00 or 2026-07-24T09:00 with optional seconds. A start with no time — or a truthy all_day cell — makes an all-day event written as VALUE=DATE with the exclusive DTEND calendars expect, so a 24th–26th conference imports as three days. A timed row ends at its end cell, else its duration_minutes, else default_duration_minutes (60). `timezone` picks floating local times (default) or UTC times with the trailing Z. `include_alarm` adds a 15-minute pop-up reminder to every event. Output is one VCALENDAR with a VEVENT per row, RFC 5545 CRLF endings, 75-octet line folding and escaped text; UIDs come from the uid column or are generated from the title, and DTSTAMP is fixed, so the same CSV always converts to the same bytes. Up to 5000 rows per run; a row with an unreadable date, a bad duration or an end before its start is reported by row number instead of silently dropped.",
        parameters = schema_json()
    ),
)]
impl CsvToIcs {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-to-ics", |a: Args| {
            gizza_ai_csv_to_ics_core::run(
                &a.csv,
                &a.timezone,
                a.default_duration_minutes.round() as i64,
                a.include_alarm,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "csv": { "type": "string", "description": "The event list as CSV text, first row naming the columns. Quoted cells (\"Lunch, with team\") are read as one value. Columns are matched case- and punctuation-insensitively: title/summary/name/event and start/start_date/begins/date are required, and end/end_date/ends, duration_minutes/duration, description/details/notes, location/place, uid/id and all_day/all-day are optional. Dates are 2026-07-24, 2026-07-24 09:00 or 2026-07-24T09:00, each with optional seconds; a start with no time makes an all-day event. Up to 5000 rows per run." },
                    "timezone": { "type": "string", "enum": ["floating", "utc"], "default": "floating", "description": "How the times in the CSV are anchored. \"floating\" (default) writes them with no zone marker, so every calendar shows 09:00 as 09:00 in whatever timezone the reader is in — right for a schedule everyone attends locally. \"utc\" treats the pasted times as already being UTC and writes them with the trailing Z, so clients convert them to each person's own zone. All-day rows are unaffected either way." },
                    "default_duration_minutes": { "type": "integer", "default": 60, "minimum": 1, "maximum": 1440, "description": "How long a timed event lasts, in minutes, when its row gives neither an end nor a duration_minutes cell. 60 by default, 1–1440 (one day). A row's own end or duration_minutes column always wins over this, and all-day rows ignore it." },
                    "include_alarm": { "type": "boolean", "default": false, "description": "Add a pop-up reminder 15 minutes before every event (a VALARM with ACTION:DISPLAY and the event title as its message). Off by default. For an all-day event this fires 15 minutes before midnight on the day it starts, so leave it off for holiday or deadline calendars." }
                },
                "required": ["csv"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
