//! gizza-ai/recurring-task-expander — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure compute —
//! the only host capability used is the clock (`SystemTime::now`), to resolve
//! the start date when the caller does not pass one.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tasks: String,
    #[serde(default)]
    start: String,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    default_rec: String,
    #[serde(default)]
    skip_weekends: Option<bool>,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("tasks").required().describe(
                "The task list to expand, one task per line. A task repeats when it carries a \
'rec:<value>' tag and may set its anchor with 'due:YYYY-MM-DD' (todo.txt style), e.g. \
'Pay rent due:2026-09-01 rec:+1m'. Recurrence values are a number plus a unit — d (days), \
b (business days, Mon-Fri), w (weeks), m (months), y (years) — such as 1d, 3w, 2m; a leading \
'+' (rec:+1m) keeps the fixed due-date schedule, while a plain value is completion-based and \
restarts from the start date when the task is overdue. Weekday patterns are also accepted: \
rec:mon, rec:mon,thu, rec:weekdays, rec:weekends. Leading list markers ('- ', '- [ ] ', '1. ') \
are stripped; priorities, +projects and @contexts are preserved; blank lines and lines starting \
with '#' are ignored. Lines without a recurrence pass through unchanged. Maximum 200 lines.",
            ),
        )
        .param(
            Param::string("start").describe(
                "The base date the expansion starts from, as YYYY-MM-DD (e.g. 2026-08-08). No \
instance before this date is emitted. Defaults to today (UTC).",
            ),
        )
        .param(
            Param::integer("count")
                .min(1.0)
                .max(100.0)
                .describe("How many dated instances to generate per recurring task (1-100, default 5)."),
        )
        .param(
            Param::string("default_rec").describe(
                "Recurrence applied to task lines that have no 'rec:' tag, using the same syntax \
as the tag (e.g. '1w', '+2m', 'mon,thu'). Leave blank to pass untagged lines through unchanged.",
            ),
        )
        .param(
            Param::boolean("skip_weekends").default(false).describe(
                "When true, an instance that lands on a Saturday or Sunday moves to the following \
Monday (two shifted occurrences that collapse onto the same Monday are emitted once). Default \
false. Explicit weekday patterns (rec:mon,thu) are never shifted.",
            ),
        )
        .param(
            Param::enumv("format", ["text", "markdown", "json", "csv"])
                .default("text")
                .describe(
                    "Output format. 'text' (default) is one plain task line per instance \
('<task> due:YYYY-MM-DD'), ready to paste back into a todo.txt file. 'markdown' is a \
'- [ ] <task> — due YYYY-MM-DD (Tue)' checklist. 'json' is an object with the start date and \
one entry per task ({ line, description, recurrence, strict, due, instances[{ date, weekday, \
line }] }). 'csv' has the header task,recurrence,instance,date,weekday.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Resolve the start date: use `start` if given, else today (UTC) from the host
/// clock. On wasm (chat block + CLI runtime) the host provides the clock import.
fn resolve_start(start: &str) -> Result<String, String> {
    let s = start.trim();
    if !s.is_empty() {
        return Ok(s.to_string());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_string())?;
    Ok(gizza_ai_recurring_task_expander_core::date_from_epoch_secs(
        now.as_secs() as i64,
    ))
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/recurring-task-expander",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Expand recurring tasks (rec:1w, rec:+2m, weekdays) into their next dated instances",
    skill(
        description = "Expand recurrence rules on a task list into the next N concrete dated \
instances. Pass the list as 'tasks', one task per line: a task repeats when it carries a \
'rec:<value>' tag and can anchor its schedule with 'due:YYYY-MM-DD' (todo.txt style). \
Recurrence values are a number plus d (days), b (business days), w (weeks), m (months) or y \
(years) — 1d, 3w, +2m — or weekday patterns such as mon, mon,thu, weekdays, weekends. A leading \
'+' means strict/due-date recurrence: the original grid is kept and past occurrences are simply \
skipped. A plain value is completion-based: an overdue task restarts from the start date. Month \
and year steps clamp to the end of short months (Jan 31 + 1m = Feb 28). Set 'start' \
(YYYY-MM-DD, default today) to expand from another date, 'count' for how many instances per \
task (1-100, default 5), 'default_rec' to apply a recurrence to untagged lines, and \
'skip_weekends' to push Saturday/Sunday instances to the Monday. Output 'format' is text \
(pasteable todo.txt lines), markdown, json or csv. Use this to turn 'Pay rent rec:+1m' into a \
real dated schedule, plan a quarter of chores, or check when a repeating task actually lands.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "recurring-task-expander", |a: Args| {
            let start = resolve_start(&a.start).map_err(SkillError::InvalidArgs)?;
            let count = a.count.unwrap_or(5);
            let fmt = if a.format.trim().is_empty() {
                "text"
            } else {
                a.format.trim()
            };
            gizza_ai_recurring_task_expander_core::expand(
                &a.tasks,
                &start,
                count,
                &a.default_rec,
                a.skip_weekends.unwrap_or(false),
                fmt,
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
                    "tasks": {
                        "type": "string",
                        "description": "The task list to expand, one task per line. A task repeats when it carries a 'rec:<value>' tag and may set its anchor with 'due:YYYY-MM-DD' (todo.txt style), e.g. 'Pay rent due:2026-09-01 rec:+1m'. Recurrence values are a number plus a unit — d (days), b (business days, Mon-Fri), w (weeks), m (months), y (years) — such as 1d, 3w, 2m; a leading '+' (rec:+1m) keeps the fixed due-date schedule, while a plain value is completion-based and restarts from the start date when the task is overdue. Weekday patterns are also accepted: rec:mon, rec:mon,thu, rec:weekdays, rec:weekends. Leading list markers ('- ', '- [ ] ', '1. ') are stripped; priorities, +projects and @contexts are preserved; blank lines and lines starting with '#' are ignored. Lines without a recurrence pass through unchanged. Maximum 200 lines."
                    },
                    "start": {
                        "type": "string",
                        "description": "The base date the expansion starts from, as YYYY-MM-DD (e.g. 2026-08-08). No instance before this date is emitted. Defaults to today (UTC)."
                    },
                    "count": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "description": "How many dated instances to generate per recurring task (1-100, default 5)."
                    },
                    "default_rec": {
                        "type": "string",
                        "description": "Recurrence applied to task lines that have no 'rec:' tag, using the same syntax as the tag (e.g. '1w', '+2m', 'mon,thu'). Leave blank to pass untagged lines through unchanged."
                    },
                    "skip_weekends": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, an instance that lands on a Saturday or Sunday moves to the following Monday (two shifted occurrences that collapse onto the same Monday are emitted once). Default false. Explicit weekday patterns (rec:mon,thu) are never shifted."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "markdown", "json", "csv"],
                        "default": "text",
                        "description": "Output format. 'text' (default) is one plain task line per instance ('<task> due:YYYY-MM-DD'), ready to paste back into a todo.txt file. 'markdown' is a '- [ ] <task> — due YYYY-MM-DD (Tue)' checklist. 'json' is an object with the start date and one entry per task ({ line, description, recurrence, strict, due, instances[{ date, weekday, line }] }). 'csv' has the header task,recurrence,instance,date,weekday."
                    }
                },
                "required": ["tasks"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The CLI/chat surface defaults: blank start resolves to a real date and
    /// the expansion runs end to end.
    #[test]
    fn resolve_start_defaults_to_today() {
        let today = resolve_start("").unwrap();
        assert_eq!(today.len(), 10, "{today}");
        assert!(gizza_ai_recurring_task_expander_core::parse_date(&today).is_ok());
        assert_eq!(resolve_start(" 2026-08-08 ").unwrap(), "2026-08-08");
    }
}
