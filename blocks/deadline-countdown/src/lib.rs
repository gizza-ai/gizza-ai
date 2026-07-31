//! gizza-ai/deadline-countdown — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tasks: String,
    now: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    include_completed: bool,
    #[serde(default = "default_soon_days")]
    soon_days: i64,
}

fn default_format() -> String {
    "table".to_string()
}
fn default_soon_days() -> i64 {
    7
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("tasks").required().describe("One task per line. Include a due date as YYYY-MM-DD, YYYY-MM-DD HH:MM, ISO datetime, or after due:/deadline:. Completed lines beginning with x, [x], done:, or ✓ are skipped unless include_completed is true."))
        .param(Param::string("now").required().describe("Reference date/time for deterministic countdowns, for example 2026-07-31 12:00 or 2026-07-31T12:00:00Z."))
        .param(Param::enumv("format", ["table", "markdown", "json", "csv"]).default("table").describe("Output format: aligned text table, Markdown table, JSON rows, or CSV rows."))
        .param(Param::boolean("include_completed").default(false).describe("Include tasks already marked complete instead of skipping them."))
        .param(Param::integer("soon_days").default(7).min(0.0).describe("Number of days ahead that should be labeled DUE SOON. Due-today and overdue statuses are always shown."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/deadline-countdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sort dated tasks by overdue and upcoming deadlines",
    skill(
        description = "Compute the time remaining for each task's due date, label overdue/today/soon/later status, and sort the list by urgency. Paste one task per line with a date (YYYY-MM-DD, YYYY-MM-DD HH:MM, ISO datetime, or due:/deadline:), provide a deterministic 'now' value, choose table/Markdown/JSON/CSV output, and optionally include completed tasks or tune the due-soon window.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "deadline-countdown", |a: Args| {
            gizza_ai_deadline_countdown_core::run(
                &a.tasks,
                &a.now,
                &a.format,
                a.include_completed,
                a.soon_days,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "tasks":{"type":"string","description":"One task per line. Include a due date as YYYY-MM-DD, YYYY-MM-DD HH:MM, ISO datetime, or after due:/deadline:. Completed lines beginning with x, [x], done:, or ✓ are skipped unless include_completed is true."},
                "now":{"type":"string","description":"Reference date/time for deterministic countdowns, for example 2026-07-31 12:00 or 2026-07-31T12:00:00Z."},
                "format":{"type":"string","enum":["table","markdown","json","csv"],"default":"table","description":"Output format: aligned text table, Markdown table, JSON rows, or CSV rows."},
                "include_completed":{"type":"boolean","default":false,"description":"Include tasks already marked complete instead of skipping them."},
                "soon_days":{"type":"integer","default":7,"minimum":0,"description":"Number of days ahead that should be labeled DUE SOON. Due-today and overdue statuses are always shown."}
            },
            "required":["tasks","now"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
