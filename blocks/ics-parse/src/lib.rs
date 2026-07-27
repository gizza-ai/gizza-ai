//! gizza-ai/ics-parse — chat skill block on the shared tool abstraction.
//! Parses an iCalendar (.ics) document into a structured JSON array of events —
//! one object per VEVENT with title, start/end, all-day flag, location,
//! description, status, categories, organizer/attendees, and a parsed RRULE.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page + query-params); handle() delegates to block_utils::run_skill and
//! the pure logic lives in gizza-ai-ics-parse-core. Pure compute — no host
//! calls, runs entirely inside the WASM sandbox; nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ics_parse_core::parse_ics_str;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ics: String,
    #[serde(default)]
    date_format: String,
    #[serde(default = "default_true")]
    pretty: bool,
    #[serde(default = "default_true")]
    include_description: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("ics")
                .required()
                .describe("The iCalendar (.ics) document to parse. Paste the whole file contents — it starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks. Folded lines are unfolded and RFC 5545 TEXT escapes are decoded. Only VEVENTs are parsed (task/journal/free-busy components are ignored)."),
        )
        .param(
            Param::enumv("date_format", ["iso", "raw", "unix"])
                .default("iso")
                .describe("How the start/end (and RRULE UNTIL) date values are written. iso (default) normalizes to ISO-8601 (20240309T081530Z → 2024-03-09T08:15:30Z, 20240309 → 2024-03-09); raw keeps the original .ics value verbatim; unix converts to epoch seconds (Z/UTC times are exact, floating/TZID times are read as wall-clock UTC — no timezone database is shipped)."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print the JSON with indentation. Default true; set false for compact single-line JSON."),
        )
        .param(
            Param::boolean("include_description")
                .default(true)
                .describe("Include each event's DESCRIPTION field. Default true; set false to drop it (handy when descriptions are long)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IcsParse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ics-parse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an iCalendar (.ics) file into a structured JSON list of events.",
    skill(
        description = "Parse an iCalendar (.ics) document into a JSON array of event objects — one per VEVENT, in document order. Each object carries uid, summary, start, end, an all_day flag (set for VALUE=DATE / date-only values), location, description, status, categories (a list), organizer and attendees (each name + email, parsed from the CN parameter and the mailto: value), and recurrence (the RRULE parsed into a structured object: freq/interval/count/until/byday/…). Empty or absent fields are omitted so the JSON stays clean. date_format writes start/end/until as iso (normalized ISO-8601, default), raw (original .ics text), or unix (epoch seconds; Z times exact, floating/TZID read as UTC). pretty toggles indented vs compact JSON; set include_description=false to drop the description field. Folded lines are unfolded, RFC 5545 TEXT escapes (\\n, \\,, \\;) are decoded, and VALARM/VTIMEZONE sub-components are skipped so their properties never leak into the event. Paste the .ics contents in the 'ics' field. Runs entirely in-browser; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl IcsParse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ics-parse", |a: Args| {
            parse_ics_str(&a.ics, &a.date_format, a.pretty, a.include_description)
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
    /// reviewed. (Regenerate this literal when the descriptor changes.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "ics": { "type": "string", "description": "The iCalendar (.ics) document to parse. Paste the whole file contents — it starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks. Folded lines are unfolded and RFC 5545 TEXT escapes are decoded. Only VEVENTs are parsed (task/journal/free-busy components are ignored)." },
                    "date_format": { "type": "string", "enum": ["iso", "raw", "unix"], "default": "iso", "description": "How the start/end (and RRULE UNTIL) date values are written. iso (default) normalizes to ISO-8601 (20240309T081530Z → 2024-03-09T08:15:30Z, 20240309 → 2024-03-09); raw keeps the original .ics value verbatim; unix converts to epoch seconds (Z/UTC times are exact, floating/TZID times are read as wall-clock UTC — no timezone database is shipped)." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print the JSON with indentation. Default true; set false for compact single-line JSON." },
                    "include_description": { "type": "boolean", "default": true, "description": "Include each event's DESCRIPTION field. Default true; set false to drop it (handy when descriptions are long)." }
                },
                "required": ["ics"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
