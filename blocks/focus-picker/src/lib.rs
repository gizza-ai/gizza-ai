//! gizza-ai/focus-picker — pick one task to do next from a pasted task list.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill and the shared core scorer.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tasks: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    today: String,
    #[serde(default = "default_priority")]
    default_priority: String,
    #[serde(default = "default_effort")]
    default_effort: f64,
    #[serde(default = "default_true")]
    overdue_boost: bool,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    show_ranking: bool,
}

fn default_method() -> String {
    "balanced".into()
}
fn default_priority() -> String {
    "p3".into()
}
fn default_effort() -> f64 {
    2.0
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "text".into()
}

fn now_unix_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("tasks").required().describe("One task per line. Add optional inline tags such as `!p1`, `due:2026-08-25`, `due:tomorrow`, `due:+3d`, `est:90m`, `est:2h` or `est:1d`; or paste pipe/tab columns as `task | priority | due | effort`. Up to 500 tasks."))
        .param(Param::enumv("method", ["balanced", "deadline", "wsjf", "quick-wins", "eisenhower"]).default("balanced").describe("Scoring method. `balanced` blends priority, due-date urgency and effort ease; `deadline` favors urgent due dates; `wsjf` divides value/urgency by job size; `quick-wins` favors small tasks; `eisenhower` ranks urgent/important quadrants."))
        .param(Param::string("today").default("").describe("Optional ISO date (YYYY-MM-DD) used as 'today' for relative due dates and reproducible examples. Leave blank to use the current date."))
        .param(Param::enumv("default_priority", ["p0", "p1", "p2", "p3", "p4"]).default("p3").describe("Priority assigned to rows that do not say `!p0` through `!p4` or include a priority column. Lower numbers mean more important; default p3."))
        .param(Param::number("default_effort").default(2.0).min(0.25).max(40.0).describe("Default effort in hours for rows without `est:` or an effort column. Effort lowers scores, especially WSJF and quick-wins. Default 2 hours."))
        .param(Param::boolean("overdue_boost").default(true).describe("When true, overdue tasks are pinned above non-overdue work before score tie-breaking. Turn off to rank everything strictly by the selected formula."))
        .param(Param::enumv("format", ["text", "markdown", "json"]).default("text").describe("Output format: readable text, a Markdown ranking table, or machine-readable JSON."))
        .param(Param::boolean("show_ranking").default(true).describe("When true, include the full ranked list after the single focus pick. Set false for a terse answer with only the top task, reason and summary."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/focus-picker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pick the one task to focus on next from priority, due date and effort cues",
    skill(
        description = "Choose the single most important next task from a pasted list, using deterministic local scoring. Each task can carry `!p0`-`!p4` priority, `due:` dates such as ISO dates, today/tomorrow, weekdays or +Nd/+Nw offsets, and `est:` effort such as 90m, 2h or 1d; pipe/tab columns are accepted too. Methods include balanced, deadline, WSJF, quick-wins and Eisenhower quadrants. The output names the top pick, explains why with score/priority/due/effort facts, optionally lists the full ranking, and can be text, Markdown or JSON. Caps: 500 tasks.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "focus-picker", |a: Args| {
            let today_days = gizza_ai_focus_picker_core::resolve_today(&a.today, now_unix_secs())
                .map_err(SkillError::InvalidArgs)?;
            let opts = gizza_ai_focus_picker_core::Options {
                tasks: &a.tasks,
                method: &a.method,
                today_days,
                default_priority: &a.default_priority,
                default_effort: a.default_effort,
                overdue_boost: a.overdue_boost,
                format: &a.format,
                show_ranking: a.show_ranking,
            };
            gizza_ai_focus_picker_core::pick(&opts).map_err(SkillError::InvalidArgs)
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
    fn descriptor_params_cover_every_arg_field() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived["properties"].as_object().unwrap();
        for name in [
            "tasks",
            "method",
            "today",
            "default_priority",
            "default_effort",
            "overdue_boost",
            "format",
            "show_ranking",
        ] {
            let p = props.get(name).unwrap_or_else(|| panic!("missing {name}"));
            assert!(p["description"].as_str().is_some_and(|d| d.len() > 30));
        }
        assert_eq!(derived["required"], serde_json::json!(["tasks"]));
        assert_eq!(
            props["method"]["enum"],
            serde_json::json!(["balanced", "deadline", "wsjf", "quick-wins", "eisenhower"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["text", "markdown", "json"])
        );
    }
}
