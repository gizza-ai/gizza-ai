//! gizza-ai/funnel-conversion-analyzer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool { true }

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    steps: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    event: String,
    #[serde(default)]
    time: String,
    #[serde(default = "default_true")]
    ordered: bool,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("Event log as CSV/table text: one row per user event, with a user-id column and an event-name column. Rows may use comma, tab, semicolon, or pipe."),
        )
        .param(
            Param::string("steps")
                .describe("Ordered funnel steps as a comma-separated list of event names (e.g. \"view,signup,purchase\"). Leave empty to auto-derive the steps from the distinct events in first-seen order."),
        )
        .param(
            Param::string("user")
                .describe("User-id column: a header name or a 1-based column index. Defaults to the first column."),
        )
        .param(
            Param::string("event")
                .describe("Event-name column: a header name or a 1-based column index. Defaults to the second column."),
        )
        .param(
            Param::string("time")
                .describe("Optional timestamp column (header name or 1-based index). When set, ordered funnels require each step to happen after the previous one in time (numeric epochs or ISO-8601 strings)."),
        )
        .param(
            Param::boolean("ordered")
                .default(true)
                .describe("When true (default), a user reaches step N only after completing every earlier step (a strict funnel). When false, each step counts every user who performed it, independently."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), the first row is treated as column names so `user`/`event`/`time` can reference them by name."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Input delimiter: comma (default), tab, semicolon, or pipe."),
        )
        .param(
            Param::enumv("format", ["table", "json"])
                .default("table")
                .describe("Output format: table (human-readable funnel summary, default) or json (structured per-step stats)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/funnel-conversion-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute funnel conversion and drop-off rates from an event CSV.",
    skill(
        description = "Analyze a funnel from an event log (CSV/table text). Given a user-id column and an event-name column, count the unique users who reach each step, and report conversion from the top step, conversion from the previous step, and the users lost (drop-off) at each step. Supply `steps` as an ordered comma-separated list of event names, or leave it empty to auto-derive the steps from the distinct events in first-seen order. With ordered=true (default) a user must complete each earlier step to reach the next; set ordered=false to count every user who performed a step independently. Provide a `time` column to enforce chronological order between steps. Choose format='table' for a readable summary or 'json' for structured per-step stats.",
        parameters = schema_json()
    )
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "funnel-conversion-analyzer", |a: Args| {
            gizza_ai_funnel_conversion_analyzer_core::analyze(
                &a.data,
                &a.steps,
                &a.user,
                &a.event,
                &a.time,
                a.ordered,
                a.header,
                &a.delimiter,
                &a.format,
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
                    "data": { "type": "string", "description": "Event log as CSV/table text: one row per user event, with a user-id column and an event-name column. Rows may use comma, tab, semicolon, or pipe." },
                    "steps": { "type": "string", "description": "Ordered funnel steps as a comma-separated list of event names (e.g. \"view,signup,purchase\"). Leave empty to auto-derive the steps from the distinct events in first-seen order." },
                    "user": { "type": "string", "description": "User-id column: a header name or a 1-based column index. Defaults to the first column." },
                    "event": { "type": "string", "description": "Event-name column: a header name or a 1-based column index. Defaults to the second column." },
                    "time": { "type": "string", "description": "Optional timestamp column (header name or 1-based index). When set, ordered funnels require each step to happen after the previous one in time (numeric epochs or ISO-8601 strings)." },
                    "ordered": { "type": "boolean", "default": true, "description": "When true (default), a user reaches step N only after completing every earlier step (a strict funnel). When false, each step counts every user who performed it, independently." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), the first row is treated as column names so `user`/`event`/`time` can reference them by name." },
                    "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Input delimiter: comma (default), tab, semicolon, or pipe." },
                    "format": { "type": "string", "enum": ["table", "json"], "default": "table", "description": "Output format: table (human-readable funnel summary, default) or json (structured per-step stats)." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
