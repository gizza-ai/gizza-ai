//! gizza-ai/kanban-board-builder — turn a task list or freeform notes into a
//! structured kanban board. Chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_kanban_board_builder_core::build;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tasks: String,
    #[serde(default = "default_columns")]
    columns: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_title")]
    title: String,
    #[serde(default)]
    default_column: String,
    #[serde(default)]
    wip_limit: i64,
    #[serde(default = "default_sort")]
    sort_by: String,
    #[serde(default = "default_true")]
    show_counts: bool,
}
fn default_columns() -> String {
    "To Do, In Progress, Done".into()
}
fn default_format() -> String {
    "markdown".into()
}
fn default_title() -> String {
    "Kanban Board".into()
}
fn default_sort() -> String {
    "none".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("tasks")
                .required()
                .describe("The task list or freeform notes, one task per line. Bullets ('-', '*', '1.') and GFM checkboxes ('- [ ]', '- [x]') are optional. Each line becomes one card; '## Heading' or 'Heading:' lines set the column for the lines beneath them."),
        )
        .param(
            Param::string("columns")
                .default("To Do, In Progress, Done")
                .describe("Comma-separated column names, in board order (max 12). Example: 'Backlog, To Do, In Progress, Blocked, Done'. Names are matched loosely, so a column called 'Doing' still catches 'in progress'."),
        )
        .param(
            Param::enumv("format", ["markdown", "table", "json"])
                .default("markdown")
                .describe("Output shape. 'markdown' (default) writes one '## Column' section per lane with a checkbox list under each; 'table' writes a Markdown table with one column per lane, side by side; 'json' returns a board object with per-column counts, WIP-limit state and parsed card fields."),
        )
        .param(
            Param::string("title")
                .default("Kanban Board")
                .describe("Board title, rendered as the '# ' heading above the columns (and as 'title' in JSON). Leave empty for no title."),
        )
        .param(
            Param::string("default_column")
                .default("")
                .describe("Column that cards with no detected status land in. Empty (default) means the first column."),
        )
        .param(
            Param::integer("wip_limit")
                .default(0)
                .min(0.0)
                .max(100.0)
                .describe("Work-in-progress limit applied to every column. 0 (default) means no limit. Columns holding more cards than this are flagged '(4 / limit 3 — over WIP limit)' in Markdown and 'over_limit: true' in JSON. Advisory only — no card is ever dropped."),
        )
        .param(
            Param::enumv("sort_by", ["none", "priority", "due", "text"])
                .default("none")
                .describe("Card order within each column. 'none' (default) keeps the order the tasks were written in; 'priority' puts critical/high first; 'due' puts the earliest due date first and undated cards last; 'text' sorts alphabetically."),
        )
        .param(
            Param::boolean("show_counts")
                .default(true)
                .describe("When true (default), append the card count to each column heading, e.g. '## In Progress (2)'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct KanbanBoardBuilder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/kanban-board-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a task list or freeform notes into a kanban board in Markdown or JSON",
    skill(
        description = "Turn a task list or freeform notes into a structured kanban board. Pass the notes as 'tasks', one task per line (bullets and '- [ ]'/'- [x]' checkboxes optional). Each line becomes a card, routed to a column by: an explicit tag (status:doing, status=done, or a bracketed [In Progress]), a #tag naming a column, the '## Heading' or 'Heading:' section it sits under, a ticked checkbox (→ the done column), or a workflow marker in the text ('in progress', 'blocked', 'waiting on', 'needs review', and one-word markers like 'wip'/'done'/'blocked' at the start or end of a line). Anything unrouted goes to 'default_column' (or the first column). Card metadata is parsed off each line: '@name' assignee, '#tag' and '[tag]' labels, priority ('!high', 'P1', 'priority:critical'), and 'due:YYYY-MM-DD'. Set 'columns' to any comma-separated lane list (default 'To Do, In Progress, Done', max 12), 'sort_by' to order cards within a lane, 'wip_limit' to flag over-full columns, and 'format' to choose the output: markdown sections (default), a side-by-side markdown table, or a JSON board object with counts and parsed fields. Max 500 tasks.",
        parameters = schema_json()
    ),
)]
impl KanbanBoardBuilder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "kanban-board-builder", |a: Args| {
            build(
                &a.tasks,
                &a.columns,
                &a.format,
                &a.title,
                &a.default_column,
                a.wip_limit,
                &a.sort_by,
                a.show_counts,
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
                    "tasks":          { "type": "string", "description": "The task list or freeform notes, one task per line. Bullets ('-', '*', '1.') and GFM checkboxes ('- [ ]', '- [x]') are optional. Each line becomes one card; '## Heading' or 'Heading:' lines set the column for the lines beneath them." },
                    "columns":        { "type": "string", "default": "To Do, In Progress, Done", "description": "Comma-separated column names, in board order (max 12). Example: 'Backlog, To Do, In Progress, Blocked, Done'. Names are matched loosely, so a column called 'Doing' still catches 'in progress'." },
                    "format":         { "type": "string", "enum": ["markdown", "table", "json"], "default": "markdown", "description": "Output shape. 'markdown' (default) writes one '## Column' section per lane with a checkbox list under each; 'table' writes a Markdown table with one column per lane, side by side; 'json' returns a board object with per-column counts, WIP-limit state and parsed card fields." },
                    "title":          { "type": "string", "default": "Kanban Board", "description": "Board title, rendered as the '# ' heading above the columns (and as 'title' in JSON). Leave empty for no title." },
                    "default_column": { "type": "string", "default": "", "description": "Column that cards with no detected status land in. Empty (default) means the first column." },
                    "wip_limit":      { "type": "integer", "default": 0, "minimum": 0, "maximum": 100, "description": "Work-in-progress limit applied to every column. 0 (default) means no limit. Columns holding more cards than this are flagged '(4 / limit 3 — over WIP limit)' in Markdown and 'over_limit: true' in JSON. Advisory only — no card is ever dropped." },
                    "sort_by":        { "type": "string", "enum": ["none", "priority", "due", "text"], "default": "none", "description": "Card order within each column. 'none' (default) keeps the order the tasks were written in; 'priority' puts critical/high first; 'due' puts the earliest due date first and undated cards last; 'text' sorts alphabetically." },
                    "show_counts":    { "type": "boolean", "default": true, "description": "When true (default), append the card count to each column heading, e.g. '## In Progress (2)'." }
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
