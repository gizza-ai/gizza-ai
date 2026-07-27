//! gizza-ai/text-to-reminders — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure Rust → runs on every
//! backend. It turns a free-form brain-dump into an iCalendar (.ics) file of
//! reminder/task components, parsing due dates/times deterministically.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_to_reminders_core::{build_reminders, civil_from_days};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    reference_date: String,
    #[serde(default = "default_true")]
    detect_priority: bool,
    #[serde(default = "default_true")]
    include_undated: bool,
    #[serde(default)]
    alarm_minutes: i64,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Parsing is deterministic: a line
/// only gains a due date when it contains a recognised date/time phrase, and a
/// priority only when it contains a priority keyword — nothing is invented.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text").required().describe(
                "Your free-form notes or brain-dump, one task per line. Each line becomes a \
                 reminder. Natural-language due phrases are parsed and stripped from the title: \
                 today, tonight, tomorrow, day after tomorrow, weekday names, next week/month, \
                 'in 3 days', 'in 2 weeks', ISO dates (2026-03-05), 3/5, 'March 5, 2027', '5 Mar', \
                 plus times like 'at 3pm', '5:30pm', '09:30', noon and midnight.",
            ),
        )
        .param(
            Param::string("reference_date").describe(
                "ISO date (YYYY-MM-DD) that relative phrases like today, tomorrow, or Monday are \
                 measured from. Leave blank to use the current date.",
            ),
        )
        .param(
            Param::boolean("detect_priority").default(true).describe(
                "Map priority keywords (urgent, asap, important, critical → high; someday, \
                 whenever → low) onto the iCalendar PRIORITY field, and drop the keyword from the \
                 title. Turn off to keep titles verbatim with no priority.",
            ),
        )
        .param(
            Param::boolean("include_undated").default(true).describe(
                "Keep lines that have no recognised date as tasks with no due date. Turn off to \
                 emit only lines that resolved to a date.",
            ),
        )
        .param(
            Param::integer("alarm_minutes").min(0.0).default(0).describe(
                "If greater than 0, attach a display reminder that triggers this many minutes \
                 before each dated task's due time. 0 adds no reminder.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// The current UTC date as `YYYY-MM-DD`, used when `reference_date` is blank.
/// `SystemTime` is available in the wafer runtime and natively in the CLI.
#[cfg(not(target_arch = "wasm32"))]
fn today_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{:04}-{:02}-{:02}", y, m, d)
}
#[cfg(target_arch = "wasm32")]
fn today_utc() -> String {
    // wafer provides the clock import; SystemTime works under wasmi too.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn run(a: Args) -> Result<String, String> {
    let reference = if a.reference_date.trim().is_empty() {
        today_utc()
    } else {
        a.reference_date.clone()
    };
    build_reminders(
        &a.text,
        &reference,
        a.detect_priority,
        a.include_undated,
        a.alarm_minutes,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-to-reminders",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn free-form notes into an iCalendar reminders file",
    skill(
        description = "Turn a free-form brain-dump into a ready-to-import iCalendar (.ics) file of reminders, one task per line. Natural-language due dates and times are parsed deterministically and stripped from each title — today, tonight, tomorrow, weekday names, next week/month, 'in 3 days', ISO dates, M/D, 'March 5, 2027', and clock times like 'at 3pm', '09:30', noon. Dates are anchored on an optional reference_date. Priority keywords map to the iCalendar PRIORITY field, undated lines can be kept as tasks with no due, and an optional alarm attaches a display reminder before each due. Pure and private: no LLM, no accounts, no upload.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-to-reminders", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
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
                    "text": { "type": "string", "description": "Your free-form notes or brain-dump, one task per line. Each line becomes a reminder. Natural-language due phrases are parsed and stripped from the title: today, tonight, tomorrow, day after tomorrow, weekday names, next week/month, 'in 3 days', 'in 2 weeks', ISO dates (2026-03-05), 3/5, 'March 5, 2027', '5 Mar', plus times like 'at 3pm', '5:30pm', '09:30', noon and midnight." },
                    "reference_date": { "type": "string", "description": "ISO date (YYYY-MM-DD) that relative phrases like today, tomorrow, or Monday are measured from. Leave blank to use the current date." },
                    "detect_priority": { "type": "boolean", "default": true, "description": "Map priority keywords (urgent, asap, important, critical → high; someday, whenever → low) onto the iCalendar PRIORITY field, and drop the keyword from the title. Turn off to keep titles verbatim with no priority." },
                    "include_undated": { "type": "boolean", "default": true, "description": "Keep lines that have no recognised date as tasks with no due date. Turn off to emit only lines that resolved to a date." },
                    "alarm_minutes": { "type": "integer", "minimum": 0, "default": 0, "description": "If greater than 0, attach a display reminder that triggers this many minutes before each dated task's due time. 0 adds no reminder." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
