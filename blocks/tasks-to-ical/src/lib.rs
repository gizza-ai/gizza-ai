//! gizza-ai/tasks-to-ical — chat skill block on the shared tool abstraction.
//! Turns a todo.txt task list into one iCalendar (.ics) document of to-do or
//! VEVENT entries. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tasks: String,
    #[serde(default = "default_component")]
    component: String,
    #[serde(default = "default_include")]
    include: String,
    #[serde(default)]
    skip_completed: bool,
    #[serde(default = "default_duration")]
    duration_minutes: f64,
    #[serde(default)]
    reminder_minutes: f64,
    #[serde(default = "default_timezone")]
    timezone: String,
    #[serde(default)]
    calendar_name: String,
}

fn default_component() -> String {
    "vtodo".to_string()
}
fn default_include() -> String {
    "dated".to_string()
}
fn default_duration() -> f64 {
    60.0
}
fn default_timezone() -> String {
    "floating".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("tasks")
                .required()
                .describe("The task list in todo.txt form, one task per line. Understood on each line: a leading `x` completion marker followed by the completion date and then the creation date, an `(A)`-`(Z)` priority (before or after the completion date, or as a `pri:A` tag), `+project` and `@context` words, and the `due:`, `t:`, `rec:`, `h:` and `uid:`/`id:` tags. Dates are `due:2026-08-25` for a whole-day deadline or `due:2026-08-25T14:30` for a timed one, with optional seconds. Blank lines and lines starting with `#` are ignored, and unknown `key:value` words stay in the task text. Up to 2000 tasks per run."),
        )
        .param(
            Param::enumv("component", ["vtodo", "vevent"])
                .default("vtodo")
                .describe("Which iCalendar component each task becomes. \"vtodo\" (default) writes to-do entries carrying DUE, STATUS and PERCENT-COMPLETE — what Apple Reminders, Nextcloud Tasks and Thunderbird import as tasks. \"vevent\" writes calendar events instead, which every calendar app shows on the grid, including Google Calendar, which ignores to-do entirely. Every task needs a date to become an event, so \"vevent\" rejects an undated task by line number rather than dropping it."),
        )
        .param(
            Param::enumv("include", ["dated", "all"])
                .default("dated")
                .describe("Which tasks to export. \"dated\" (default) keeps only tasks carrying a `due:` or `t:` tag, so a working todo.txt full of someday items converts to just the deadlines. \"all\" keeps every task; undated ones become to-do entries with no DUE, which task apps show in an unscheduled list."),
        )
        .param(
            Param::boolean("skip_completed")
                .default(false)
                .describe("Leave out tasks marked done with a leading `x`. Off by default, because a completed task still exports usefully — it becomes STATUS:COMPLETED with PERCENT-COMPLETE:100 and its COMPLETED date, which is how you archive finished work into a calendar. Turn it on to export only what is still outstanding."),
        )
        .param(
            Param::integer("duration_minutes")
                .default(60)
                .min(1.0)
                .max(1440.0)
                .describe("How long a timed calendar event lasts, in minutes, when `component` is \"vevent\" and the task's date carries a time (`due:2026-08-25T14:30`). 60 by default, 1-1440 (one day). Whole-day tasks ignore it: they span from their `t:` date through their `due:` date as an all-day event, and to-do output ignores it entirely."),
        )
        .param(
            Param::integer("reminder_minutes")
                .default(0)
                .min(0.0)
                .max(10080.0)
                .describe("Add a pop-up reminder this many minutes ahead of time (a VALARM with ACTION:DISPLAY and the task text as its message). 0 by default, which writes no reminder at all; up to 10080 minutes (one week) of lead time. On a to-do the trigger is anchored to the DUE date with RELATED=END, per RFC 5545; on a VEVENT it counts back from the event's start."),
        )
        .param(
            Param::enumv("timezone", ["floating", "utc"])
                .default("floating")
                .describe("How the times on timed tasks are anchored. \"floating\" (default) writes them with no zone marker, so every calendar shows 14:30 as 14:30 in whatever timezone the reader is in — right for a personal deadline list. \"utc\" treats the written times as already being UTC and adds the trailing Z, so clients convert them into each person's own zone. Whole-day tasks are unaffected either way."),
        )
        .param(
            Param::string("calendar_name")
                .describe("An optional display name for the whole calendar, written as X-WR-CALNAME — Google Calendar, Apple Calendar and Outlook use it to label the imported calendar instead of showing the file name. Leave it empty to write no name at all."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TasksToIcal;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/tasks-to-ical",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a todo.txt task list into an importable iCalendar (.ics) file of to-do or VEVENT entries",
    skill(
        description = "Turn a todo.txt task list into an iCalendar (.ics) file that Apple Reminders, Nextcloud Tasks, Thunderbird, Google Calendar, Outlook and anything else that speaks iCalendar can import. Paste the list into `tasks`, one task per line. Each line is read as todo.txt: a leading `x` marks it done and is followed by the completion date and then the creation date, `(A)`-`(Z)` sets priority (also accepted as a `pri:A` tag), `+project` and `@context` words become CATEGORIES, and `due:` / `t:` carry the deadline and the start (threshold) date. Dates are `due:2026-08-25` for a whole day or `due:2026-08-25T14:30` for a timed deadline. `component` picks to-do entries (default — DUE, STATUS:NEEDS-ACTION or COMPLETED, PERCENT-COMPLETE, the COMPLETED and CREATED dates) or VEVENT entries, which land on the calendar grid in apps that ignore to-do; an event runs from its `t:` date through its `due:` date as an all-day span, or for `duration_minutes` when the date is timed. `include` exports only dated tasks (default) or all of them, and `skip_completed` drops finished ones. `reminder_minutes` adds a VALARM (anchored to DUE with RELATED=END on a to-do, to the start on a VEVENT), `timezone` picks floating local times or UTC with the trailing Z, and `calendar_name` sets X-WR-CALNAME. The task text is cleaned of its priority, dates and known tags for SUMMARY while the original line is kept verbatim in DESCRIPTION, so nothing is lost; unknown `key:value` words stay in the text. UIDs come from a `uid:`/`id:` tag or are derived from the task, and DTSTAMP is fixed, so the same list always converts to the same bytes. Output is one VCALENDAR with RFC 5545 CRLF endings, 75-octet line folding and escaped text. `rec:` repeat tags are recognised and removed from the summary but NOT expanded into RRULEs — expand them first with the recurring-task-expander tool. Up to 2000 tasks per run; a task with an unreadable date, a start after its due date, or no text at all is reported by line number instead of being silently dropped.",
        parameters = schema_json()
    ),
)]
impl TasksToIcal {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "tasks-to-ical", |a: Args| {
            gizza_ai_tasks_to_ical_core::run(
                &a.tasks,
                &a.component,
                &a.include,
                a.skip_completed,
                a.duration_minutes.round() as i64,
                a.reminder_minutes.round() as i64,
                &a.timezone,
                &a.calendar_name,
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
                    "tasks": { "type": "string", "description": "The task list in todo.txt form, one task per line. Understood on each line: a leading `x` completion marker followed by the completion date and then the creation date, an `(A)`-`(Z)` priority (before or after the completion date, or as a `pri:A` tag), `+project` and `@context` words, and the `due:`, `t:`, `rec:`, `h:` and `uid:`/`id:` tags. Dates are `due:2026-08-25` for a whole-day deadline or `due:2026-08-25T14:30` for a timed one, with optional seconds. Blank lines and lines starting with `#` are ignored, and unknown `key:value` words stay in the task text. Up to 2000 tasks per run." },
                    "component": { "type": "string", "enum": ["vtodo", "vevent"], "default": "vtodo", "description": "Which iCalendar component each task becomes. \"vtodo\" (default) writes to-do entries carrying DUE, STATUS and PERCENT-COMPLETE — what Apple Reminders, Nextcloud Tasks and Thunderbird import as tasks. \"vevent\" writes calendar events instead, which every calendar app shows on the grid, including Google Calendar, which ignores to-do entirely. Every task needs a date to become an event, so \"vevent\" rejects an undated task by line number rather than dropping it." },
                    "include": { "type": "string", "enum": ["dated", "all"], "default": "dated", "description": "Which tasks to export. \"dated\" (default) keeps only tasks carrying a `due:` or `t:` tag, so a working todo.txt full of someday items converts to just the deadlines. \"all\" keeps every task; undated ones become to-do entries with no DUE, which task apps show in an unscheduled list." },
                    "skip_completed": { "type": "boolean", "default": false, "description": "Leave out tasks marked done with a leading `x`. Off by default, because a completed task still exports usefully — it becomes STATUS:COMPLETED with PERCENT-COMPLETE:100 and its COMPLETED date, which is how you archive finished work into a calendar. Turn it on to export only what is still outstanding." },
                    "duration_minutes": { "type": "integer", "default": 60, "minimum": 1, "maximum": 1440, "description": "How long a timed calendar event lasts, in minutes, when `component` is \"vevent\" and the task's date carries a time (`due:2026-08-25T14:30`). 60 by default, 1-1440 (one day). Whole-day tasks ignore it: they span from their `t:` date through their `due:` date as an all-day event, and to-do output ignores it entirely." },
                    "reminder_minutes": { "type": "integer", "default": 0, "minimum": 0, "maximum": 10080, "description": "Add a pop-up reminder this many minutes ahead of time (a VALARM with ACTION:DISPLAY and the task text as its message). 0 by default, which writes no reminder at all; up to 10080 minutes (one week) of lead time. On a to-do the trigger is anchored to the DUE date with RELATED=END, per RFC 5545; on a VEVENT it counts back from the event's start." },
                    "timezone": { "type": "string", "enum": ["floating", "utc"], "default": "floating", "description": "How the times on timed tasks are anchored. \"floating\" (default) writes them with no zone marker, so every calendar shows 14:30 as 14:30 in whatever timezone the reader is in — right for a personal deadline list. \"utc\" treats the written times as already being UTC and adds the trailing Z, so clients convert them into each person's own zone. Whole-day tasks are unaffected either way." },
                    "calendar_name": { "type": "string", "description": "An optional display name for the whole calendar, written as X-WR-CALNAME — Google Calendar, Apple Calendar and Outlook use it to label the imported calendar instead of showing the file name. Leave it empty to write no name at all." }
                },
                "required": ["tasks"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
