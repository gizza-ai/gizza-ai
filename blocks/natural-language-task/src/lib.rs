//! gizza-ai/natural-language-task — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure Rust → runs on every
//! backend. It turns a plain-English task sentence into a todo.txt line,
//! extracting priority, +projects, @contexts and a due: date deterministically.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_natural_language_task_core::{civil_from_days, to_todo_txt};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    reference_date: String,
    #[serde(default = "default_true")]
    add_creation_date: bool,
    #[serde(default = "default_true")]
    detect_priority: bool,
    #[serde(default = "default_true")]
    detect_due: bool,
    #[serde(default)]
    project: String,
    #[serde(default)]
    context: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Parsing is deterministic: a task
/// gains a `due:` date only when it contains a recognised date phrase, and a
/// priority only when a priority cue is present — nothing is invented.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text").required().describe(
                "The plain-English task, e.g. \"Call the plumber urgent tomorrow +house @phone\". \
                 Put one task per line to convert a whole brain-dump at once. Inline todo.txt tags \
                 (+project, @context) are kept as written; leading bullets/checkboxes/numbering are \
                 stripped. Recognised date phrases become a due: date and are removed from the \
                 title: today, tonight, tomorrow, day after tomorrow, weekday names (with 'next'/\
                 'this'), next week/month, 'in 3 days', 'in 2 weeks', ISO dates (2026-08-01), M/D, \
                 and 'March 5, 2027'.",
            ),
        )
        .param(
            Param::string("reference_date").describe(
                "ISO date (YYYY-MM-DD) that relative phrases like tomorrow, next Friday, or 'in 3 \
                 days' are measured from, and the creation date stamped when add_creation_date is \
                 on. Leave blank to use the current date.",
            ),
        )
        .param(
            Param::boolean("add_creation_date").default(true).describe(
                "Prefix each line with the reference date as the todo.txt creation date (after any \
                 priority), e.g. '(A) 2026-08-01 call bob'. Turn off to omit the creation date.",
            ),
        )
        .param(
            Param::boolean("detect_priority").default(true).describe(
                "Map priority cues onto a leading todo.txt priority: urgent/asap/critical/important/\
                 emergency/'high priority' -> (A); low priority/someday/whenever/minor -> (C); \
                 Todoist-style p1-p4 -> (A)-(D); an explicit '(A)'-'(D)' is kept. The cue is dropped \
                 from the title. Turn off to leave titles verbatim with no priority.",
            ),
        )
        .param(
            Param::boolean("detect_due").default(true).describe(
                "Parse a natural-language date phrase into a 'due:YYYY-MM-DD' key and strip it from \
                 the title. Turn off to keep the date words in the text and add no due: date.",
            ),
        )
        .param(
            Param::string("project").describe(
                "Default +project to append to any line that has no +project of its own (a leading \
                 '+' is optional; spaces become hyphens). Leave blank to add none.",
            ),
        )
        .param(
            Param::string("context").describe(
                "Default @context to append to any line that has no @context of its own (a leading \
                 '@' is optional; spaces become hyphens). Leave blank to add none.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// The current UTC date as `YYYY-MM-DD`, used when `reference_date` is blank.
/// `SystemTime` is available in the wafer runtime and natively in the CLI.
fn today_utc() -> String {
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
    to_todo_txt(
        &a.text,
        &reference,
        a.add_creation_date,
        a.detect_priority,
        a.detect_due,
        &a.project,
        &a.context,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/natural-language-task",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a plain-English sentence into a todo.txt task line",
    skill(
        description = "Turn a plain-English task sentence into a todo.txt line, extracting a priority, +projects, @contexts and a due: date. Priority cues (urgent, asap, important, p1-p4, an explicit (A)) become a leading '(A)'-'(D)'; natural-language dates (tomorrow, next Friday, in 3 days, ISO dates, 'March 5') become 'due:YYYY-MM-DD'; inline +project/@context tags are preserved and an optional default project/context can be appended. Put one task per line to convert a whole brain-dump into a todo.txt list at once, anchored on an optional reference_date. Pure and private: deterministic parsing, no LLM, no accounts, no upload.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "natural-language-task", |a: Args| {
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
                    "text": { "type": "string", "description": "The plain-English task, e.g. \"Call the plumber urgent tomorrow +house @phone\". Put one task per line to convert a whole brain-dump at once. Inline todo.txt tags (+project, @context) are kept as written; leading bullets/checkboxes/numbering are stripped. Recognised date phrases become a due: date and are removed from the title: today, tonight, tomorrow, day after tomorrow, weekday names (with 'next'/'this'), next week/month, 'in 3 days', 'in 2 weeks', ISO dates (2026-08-01), M/D, and 'March 5, 2027'." },
                    "reference_date": { "type": "string", "description": "ISO date (YYYY-MM-DD) that relative phrases like tomorrow, next Friday, or 'in 3 days' are measured from, and the creation date stamped when add_creation_date is on. Leave blank to use the current date." },
                    "add_creation_date": { "type": "boolean", "default": true, "description": "Prefix each line with the reference date as the todo.txt creation date (after any priority), e.g. '(A) 2026-08-01 call bob'. Turn off to omit the creation date." },
                    "detect_priority": { "type": "boolean", "default": true, "description": "Map priority cues onto a leading todo.txt priority: urgent/asap/critical/important/emergency/'high priority' -> (A); low priority/someday/whenever/minor -> (C); Todoist-style p1-p4 -> (A)-(D); an explicit '(A)'-'(D)' is kept. The cue is dropped from the title. Turn off to leave titles verbatim with no priority." },
                    "detect_due": { "type": "boolean", "default": true, "description": "Parse a natural-language date phrase into a 'due:YYYY-MM-DD' key and strip it from the title. Turn off to keep the date words in the text and add no due: date." },
                    "project": { "type": "string", "description": "Default +project to append to any line that has no +project of its own (a leading '+' is optional; spaces become hyphens). Leave blank to add none." },
                    "context": { "type": "string", "description": "Default @context to append to any line that has no @context of its own (a leading '@' is optional; spaces become hyphens). Leave blank to add none." }
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
