//! gizza-ai/issue-export-reporter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Turns a pasted Jira or
//! Linear CSV issue export into flow metrics: status breakdown, lead/cycle-time
//! percentiles, velocity per sprint or period, and an arrivals-vs-departures
//! burndown. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_issue_export_reporter_core::{
    DEFAULT_CANCELLED_STATUSES, DEFAULT_DONE_STATUSES, DEFAULT_PERCENTILES,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    /// Every option below falls back to the core's documented default when it
    /// arrives empty, so an omitted argument and a blank page field agree.
    #[serde(default)]
    report: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    columns: String,
    #[serde(default)]
    done_statuses: String,
    #[serde(default)]
    cancelled_statuses: String,
    #[serde(default)]
    group_by: String,
    #[serde(default)]
    period: String,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    business_days: bool,
    #[serde(default)]
    percentiles: String,
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV issue export, pasted whole including the header row. Jira exports (Issue key, Status, Created, Resolved, Story Points, repeated Sprint columns) and Linear exports (ID, Status, Created, Started, Completed, Estimate, Cycle Name) are recognized automatically; any other CSV works as long as it has a status column or a completion-date column. Column names are matched case-insensitively, ignoring spaces and punctuation. Timestamps may be ISO (2024-01-05, 2024-01-05T09:07:33Z, with or without an offset) or Jira-style (05/Jan/24 9:07 AM). Up to 5 MB and 50,000 rows."),
        )
        .param(
            Param::enumv(
                "report",
                ["summary", "status", "cycle_time", "velocity", "burndown", "items"],
            )
            .default("summary")
            .describe("Which report to render. 'summary' (default) combines an overview, the status breakdown, the lead/cycle-time distributions and velocity; 'status' is the per-status count/share/points table; 'cycle_time' is the lead-time (created→resolved) and, when a start column exists, cycle-time (started→resolved) distributions; 'velocity' is completed issues and points per sprint or period plus per-period averages; 'burndown' is created vs completed vs net vs open-at-end per calendar period; 'items' is the per-issue table sorted slowest first."),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv", "markdown"])
                .default("text")
                .describe("Output format. 'text' (default) is aligned plain-text tables; 'markdown' is pipe tables; 'csv' is spreadsheet-ready (multi-table reports are separated by a '# <title>' comment row); 'json' is a structured object with report, source, the detected column mapping, one section per table and any notes."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"])
                .default("auto")
                .describe("Field delimiter of the pasted export. 'auto' (default) picks whichever of comma, semicolon, tab or pipe appears most often in the header row."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Optional column mapping for exports whose headers are not auto-detected, as a comma-separated list of field=header pairs, e.g. \"status=State, resolved=Done At, points=Estimate\". Fields: key, summary, status, created, started, resolved, points, sprint, assignee, type, priority. The header may also be given as a 1-based column number. Empty (default) means use the auto-detected columns."),
        )
        .param(
            Param::string("done_statuses")
                .default(DEFAULT_DONE_STATUSES)
                .describe("Comma-separated statuses that count as finished work, matched case-insensitively. An issue also counts as finished when it carries a resolved/completed date. Empty means the default list: done, closed, resolved, completed, shipped, released, merged."),
        )
        .param(
            Param::string("cancelled_statuses")
                .default(DEFAULT_CANCELLED_STATUSES)
                .describe("Comma-separated statuses for dropped work, matched case-insensitively. Cancelled issues are excluded from completion rate, velocity and the cycle-time distributions. Empty means the default list: cancelled, canceled, won't do, wont do, duplicate, rejected, invalid."),
        )
        .param(
            Param::enumv(
                "group_by",
                ["none", "assignee", "type", "priority", "sprint", "status"],
            )
            .default("none")
            .describe("Split the lead/cycle-time distributions by a field instead of reporting one row for all issues. Default 'none'. Groups are listed largest first; issues with a blank value are grouped as (unassigned)/(no sprint)/(none)."),
        )
        .param(
            Param::enumv("period", ["auto", "sprint", "week", "month", "day"])
                .default("auto")
                .describe("Bucket size for velocity and burndown. 'auto' (default) uses sprints when the export has a Sprint/Cycle column, otherwise weeks; burndown always uses calendar buckets because sprint names have no calendar order. Weeks start on Monday."),
        )
        .param(
            Param::enumv("unit", ["days", "hours"])
                .default("days")
                .describe("Unit for every elapsed time. Default 'days'; use 'hours' for short-lived work such as support tickets."),
        )
        .param(
            Param::boolean("business_days")
                .default(false)
                .describe("Count only Monday–Friday time in lead and cycle times, dropping Saturdays and Sundays. Default false (calendar time). Public holidays are not modelled."),
        )
        .param(
            Param::string("percentiles")
                .default(DEFAULT_PERCENTILES)
                .describe("Comma-separated percentiles to report for lead and cycle time, each between 1 and 99, at most 6 (e.g. \"30,50,70,85,95\"). Empty means the default 50,85,95. Nearest-rank method: the smallest observed value at or below which at least p% of the issues fall. Min, max and mean are always reported alongside."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/issue-export-reporter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a Jira or Linear CSV issue export into status, cycle-time, velocity and burndown reports.",
    skill(
        description = "Parse a pasted Jira or Linear CSV issue export and report the team's flow metrics. Auto-detects the export dialect and its columns (key, status, created, started, resolved, story points, sprint/cycle, assignee, type, priority), reading ISO and Jira-style timestamps. Reports: an overview (issues, completed, open, cancelled, completion rate, points, created range), a status breakdown with counts/share/points, lead time (created to resolved) and cycle time (started to resolved) as min/percentiles/max/mean, velocity per sprint or calendar period with averages, an arrivals-vs-departures burndown, and a per-issue table sorted slowest first. Percentiles default to 50,85,95 and are configurable; distributions can be grouped by assignee, type, priority, sprint or status; elapsed time can be measured in days or hours and can exclude weekends. Which statuses count as done or cancelled is configurable, and non-standard exports can be mapped with columns='field=header'. Output as text, markdown, csv or json. Only the timestamps present in the export are used — a plain CSV has no transition history, so time-in-status is not available. Runs entirely in the sandbox: no tracker connection, no upload, no network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "issue-export-reporter", |a: Args| {
            gizza_ai_issue_export_reporter_core::run(
                &a.data,
                &a.report,
                &a.format,
                &a.delimiter,
                &a.columns,
                &a.done_statuses,
                &a.cancelled_statuses,
                &a.group_by,
                &a.period,
                &a.unit,
                a.business_days,
                &a.percentiles,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV issue export, pasted whole including the header row. Jira exports (Issue key, Status, Created, Resolved, Story Points, repeated Sprint columns) and Linear exports (ID, Status, Created, Started, Completed, Estimate, Cycle Name) are recognized automatically; any other CSV works as long as it has a status column or a completion-date column. Column names are matched case-insensitively, ignoring spaces and punctuation. Timestamps may be ISO (2024-01-05, 2024-01-05T09:07:33Z, with or without an offset) or Jira-style (05/Jan/24 9:07 AM). Up to 5 MB and 50,000 rows." },
                    "report": { "type": "string", "enum": ["summary", "status", "cycle_time", "velocity", "burndown", "items"], "default": "summary", "description": "Which report to render. 'summary' (default) combines an overview, the status breakdown, the lead/cycle-time distributions and velocity; 'status' is the per-status count/share/points table; 'cycle_time' is the lead-time (created→resolved) and, when a start column exists, cycle-time (started→resolved) distributions; 'velocity' is completed issues and points per sprint or period plus per-period averages; 'burndown' is created vs completed vs net vs open-at-end per calendar period; 'items' is the per-issue table sorted slowest first." },
                    "format": { "type": "string", "enum": ["text", "json", "csv", "markdown"], "default": "text", "description": "Output format. 'text' (default) is aligned plain-text tables; 'markdown' is pipe tables; 'csv' is spreadsheet-ready (multi-table reports are separated by a '# <title>' comment row); 'json' is a structured object with report, source, the detected column mapping, one section per table and any notes." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "semicolon", "tab", "pipe"], "default": "auto", "description": "Field delimiter of the pasted export. 'auto' (default) picks whichever of comma, semicolon, tab or pipe appears most often in the header row." },
                    "columns": { "type": "string", "default": "", "description": "Optional column mapping for exports whose headers are not auto-detected, as a comma-separated list of field=header pairs, e.g. \"status=State, resolved=Done At, points=Estimate\". Fields: key, summary, status, created, started, resolved, points, sprint, assignee, type, priority. The header may also be given as a 1-based column number. Empty (default) means use the auto-detected columns." },
                    "done_statuses": { "type": "string", "default": "done,closed,resolved,completed,shipped,released,merged", "description": "Comma-separated statuses that count as finished work, matched case-insensitively. An issue also counts as finished when it carries a resolved/completed date. Empty means the default list: done, closed, resolved, completed, shipped, released, merged." },
                    "cancelled_statuses": { "type": "string", "default": "cancelled,canceled,won't do,wont do,duplicate,rejected,invalid", "description": "Comma-separated statuses for dropped work, matched case-insensitively. Cancelled issues are excluded from completion rate, velocity and the cycle-time distributions. Empty means the default list: cancelled, canceled, won't do, wont do, duplicate, rejected, invalid." },
                    "group_by": { "type": "string", "enum": ["none", "assignee", "type", "priority", "sprint", "status"], "default": "none", "description": "Split the lead/cycle-time distributions by a field instead of reporting one row for all issues. Default 'none'. Groups are listed largest first; issues with a blank value are grouped as (unassigned)/(no sprint)/(none)." },
                    "period": { "type": "string", "enum": ["auto", "sprint", "week", "month", "day"], "default": "auto", "description": "Bucket size for velocity and burndown. 'auto' (default) uses sprints when the export has a Sprint/Cycle column, otherwise weeks; burndown always uses calendar buckets because sprint names have no calendar order. Weeks start on Monday." },
                    "unit": { "type": "string", "enum": ["days", "hours"], "default": "days", "description": "Unit for every elapsed time. Default 'days'; use 'hours' for short-lived work such as support tickets." },
                    "business_days": { "type": "boolean", "default": false, "description": "Count only Monday–Friday time in lead and cycle times, dropping Saturdays and Sundays. Default false (calendar time). Public holidays are not modelled." },
                    "percentiles": { "type": "string", "default": "50,85,95", "description": "Comma-separated percentiles to report for lead and cycle time, each between 1 and 99, at most 6 (e.g. \"30,50,70,85,95\"). Empty means the default 50,85,95. Nearest-rank method: the smallest observed value at or below which at least p% of the issues fall. Min, max and mean are always reported alongside." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
