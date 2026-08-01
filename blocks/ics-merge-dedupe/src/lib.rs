//! gizza-ai/ics-merge-dedupe — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page + query-params); handle() delegates to block_utils::run_skill.
//! Pure compute — no host calls, runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ics_merge_dedupe_core::merge_str;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ics: String,
    #[serde(default)]
    dedupe_by: String,
    #[serde(default)]
    keep: String,
    #[serde(default = "default_true")]
    sort: bool,
    #[serde(default)]
    calendar_name: String,
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
                .describe("The iCalendar (.ics) data to merge. Paste the whole contents of one file, or concatenate several files (each starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks) to merge them into one calendar. Folded lines and RFC 5545 escapes are handled; every event is preserved verbatim."),
        )
        .param(
            Param::enumv("dedupe_by", ["smart", "uid_start", "start_title"])
                .default("smart")
                .describe("How duplicate events are matched. smart (default) uses the UID when present, else falls back to normalized start+title; uid_start matches UID + start time (keeps recurrence overrides that share a UID but differ in start); start_title ignores UID and matches normalized start + title (collapses the same public event exported by different apps under different UIDs)."),
        )
        .param(
            Param::enumv("keep", ["first", "last_modified"])
                .default("first")
                .describe("Which member of a duplicate group survives. first (default) keeps the earliest occurrence in document order; last_modified keeps the one with the newest LAST-MODIFIED (falling back to DTSTAMP), so a later edit wins over the original."),
        )
        .param(
            Param::boolean("sort")
                .default(true)
                .describe("Sort the merged events chronologically by start time. Default true; set false to keep them in the order they appeared across the input files. Events with an unreadable start are placed last."),
        )
        .param(
            Param::string("calendar_name")
                .describe("Optional name for the merged calendar, written as an X-WR-CALNAME property (shown as the calendar title in Google/Apple/Outlook). Leave blank to omit it."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IcsMergeDedupe;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ics-merge-dedupe",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge several .ics calendars into one and drop duplicate events.",
    skill(
        description = "Merge one or more iCalendar (.ics) documents into a single VCALENDAR and remove duplicate VEVENTs. Paste the contents of one file or concatenate several files in the 'ics' field to merge them. dedupe_by chooses how duplicates are matched: smart (UID when present, else start+title), uid_start (UID + start time), or start_title (start + title, ignoring UID — for the same event exported by different apps). keep decides which of a duplicate group survives: first (document order) or last_modified (newest LAST-MODIFIED/DTSTAMP wins). sort orders the merged events chronologically by start. calendar_name optionally sets X-WR-CALNAME. Every event is preserved verbatim (line endings normalized to CRLF); identical VTIMEZONE blocks are emitted once by TZID and non-event calendar components such as tasks, journals, and free/busy blocks pass through unchanged. Recurrence rules (RRULE) are not expanded. Runs entirely in-browser; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl IcsMergeDedupe {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ics-merge-dedupe", |a: Args| {
            merge_str(&a.ics, &a.dedupe_by, &a.keep, a.sort, &a.calendar_name)
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
                    "ics": { "type": "string", "description": "The iCalendar (.ics) data to merge. Paste the whole contents of one file, or concatenate several files (each starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks) to merge them into one calendar. Folded lines and RFC 5545 escapes are handled; every event is preserved verbatim." },
                    "dedupe_by": { "type": "string", "enum": ["smart", "uid_start", "start_title"], "default": "smart", "description": "How duplicate events are matched. smart (default) uses the UID when present, else falls back to normalized start+title; uid_start matches UID + start time (keeps recurrence overrides that share a UID but differ in start); start_title ignores UID and matches normalized start + title (collapses the same public event exported by different apps under different UIDs)." },
                    "keep": { "type": "string", "enum": ["first", "last_modified"], "default": "first", "description": "Which member of a duplicate group survives. first (default) keeps the earliest occurrence in document order; last_modified keeps the one with the newest LAST-MODIFIED (falling back to DTSTAMP), so a later edit wins over the original." },
                    "sort": { "type": "boolean", "default": true, "description": "Sort the merged events chronologically by start time. Default true; set false to keep them in the order they appeared across the input files. Events with an unreadable start are placed last." },
                    "calendar_name": { "type": "string", "description": "Optional name for the merged calendar, written as an X-WR-CALNAME property (shown as the calendar title in Google/Apple/Outlook). Leave blank to omit it." }
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
