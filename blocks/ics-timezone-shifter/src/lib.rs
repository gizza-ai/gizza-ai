//! gizza-ai/ics-timezone-shifter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI and page query params); handle() delegates to the pure core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ics_timezone_shifter_core::shift_str;
use serde::Deserialize;
use wafer_sdk::*;

fn default_to() -> String {
    "UTC".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    from: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    write_as: String,
    #[serde(default = "default_true")]
    include_vtimezone: bool,
}

/// Single source for the chat schema (and CLI/page query params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The iCalendar (.ics) text to rewrite. Paste a full VCALENDAR or one or more bare VEVENT blocks. Timed DTSTART, DTEND, DUE, RECURRENCE-ID, EXDATE, RDATE, and RRULE UNTIL values are shifted; all-day VALUE=DATE entries, DTSTAMP, CREATED, LAST-MODIFIED, alarms, summaries, attendees, and unknown properties pass through. Max 5,000 VEVENT blocks per run."),
        )
        .param(
            Param::string("from")
                .default("")
                .describe("Fallback source timezone for floating date-times that do not already carry a TZID or trailing Z. Use an IANA name such as America/New_York, Europe/Berlin, Asia/Tokyo, or UTC. Blank means UTC. Existing TZID values in the calendar override this in convert mode."),
        )
        .param(
            Param::string("to")
                .default("UTC")
                .describe("Target timezone to write into, as an IANA timezone name such as UTC, Europe/Berlin, America/Los_Angeles, or Asia/Tokyo. Matching is case-insensitive for known zone names."),
        )
        .param(
            Param::enumv("mode", ["convert", "relabel"])
                .default("convert")
                .describe("How to interpret the input times: convert preserves the same instant and expresses it in the target zone; relabel keeps the wall-clock digits and declares them to be in the target zone, which repairs calendars exported with the wrong timezone stamped on them."),
        )
        .param(
            Param::enumv("write_as", ["tzid", "utc", "floating"])
                .default("tzid")
                .describe("How converted timed values are written: tzid emits local values with TZID=<target> and, by default, a fresh VTIMEZONE; utc writes UTC Z values; floating writes zone-less local values. All-day VALUE=DATE fields stay unchanged in every mode."),
        )
        .param(
            Param::boolean("include_vtimezone")
                .default(true)
                .describe("When write_as is tzid, include a generated VTIMEZONE for the target zone using chrono-tz DST transitions for the calendar's year span. Turn off only if your downstream system already supplies matching timezone definitions."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ics-timezone-shifter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shift every timed event in an .ics calendar from one timezone to another.",
    skill(
        description = "Rewrite an iCalendar (.ics) file so every timed event, task, journal, and free/busy date-time is expressed in a target timezone. The tool preserves all-day events, metadata timestamps, alarms, attendees and unknown properties; converts DTSTART, DTEND, DUE, RECURRENCE-ID, EXDATE, RDATE and RRULE UNTIL; removes stale VTIMEZONE blocks; and can emit target TZID values with a fresh VTIMEZONE, UTC Z values, or floating local values. Use convert mode to preserve instants, or relabel mode to fix an export that used the wrong timezone label.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ics-timezone-shifter", |a: Args| {
            shift_str(
                &a.input,
                &a.from,
                &a.to,
                &a.mode,
                &a.write_as,
                a.include_vtimezone,
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
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "The iCalendar (.ics) text to rewrite. Paste a full VCALENDAR or one or more bare VEVENT blocks. Timed DTSTART, DTEND, DUE, RECURRENCE-ID, EXDATE, RDATE, and RRULE UNTIL values are shifted; all-day VALUE=DATE entries, DTSTAMP, CREATED, LAST-MODIFIED, alarms, summaries, attendees, and unknown properties pass through. Max 5,000 VEVENT blocks per run." },
                "from": { "type": "string", "default": "", "description": "Fallback source timezone for floating date-times that do not already carry a TZID or trailing Z. Use an IANA name such as America/New_York, Europe/Berlin, Asia/Tokyo, or UTC. Blank means UTC. Existing TZID values in the calendar override this in convert mode." },
                "to": { "type": "string", "default": "UTC", "description": "Target timezone to write into, as an IANA timezone name such as UTC, Europe/Berlin, America/Los_Angeles, or Asia/Tokyo. Matching is case-insensitive for known zone names." },
                "mode": { "type": "string", "enum": ["convert", "relabel"], "default": "convert", "description": "How to interpret the input times: convert preserves the same instant and expresses it in the target zone; relabel keeps the wall-clock digits and declares them to be in the target zone, which repairs calendars exported with the wrong timezone stamped on them." },
                "write_as": { "type": "string", "enum": ["tzid", "utc", "floating"], "default": "tzid", "description": "How converted timed values are written: tzid emits local values with TZID=<target> and, by default, a fresh VTIMEZONE; utc writes UTC Z values; floating writes zone-less local values. All-day VALUE=DATE fields stay unchanged in every mode." },
                "include_vtimezone": { "type": "boolean", "default": true, "description": "When write_as is tzid, include a generated VTIMEZONE for the target zone using chrono-tz DST transitions for the calendar's year span. Turn off only if your downstream system already supplies matching timezone definitions." }
            },
            "required": ["input"],
            "additionalProperties": false
        });
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
