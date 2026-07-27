//! gizza-ai/critical-path-calculator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tasks: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("tasks")
                .required()
                .describe("The task list, one task per line as `name, duration[, predecessor, ...]`. `duration` is a number, or a PERT three-point estimate `optimistic/most-likely/pessimistic` (e.g. `2/4/9`, reduced to (o+4m+p)/6). Predecessors are the names of tasks that must finish first. Example: `A, 3` then `B, 4, A` then `C, 2, A` then `D, 5, B, C`. Blank lines and lines starting with `#` are ignored."),
        )
        .param(
            Param::enumv("format", ["report", "json"])
                .default("report")
                .describe("Output format: 'report' (a human-readable table of earliest/latest start & finish, total & free float, and the critical path) or 'json' (a machine-readable object with the same fields)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/critical-path-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find the critical path, project duration, and earliest/latest start times and slack for a task graph.",
    skill(
        description = "Run the Critical Path Method (CPM) on a task graph. Provide `tasks` (one per line as `name, duration[, predecessor, ...]`; duration may be a number or a PERT `optimistic/most-likely/pessimistic` estimate like `2/4/9`) and choose `format` = report | json. Returns the total project duration, the critical path, and for every task its earliest start/finish, latest start/finish, total float (slack), free float, and whether it is critical. Errors on cyclic or unknown dependencies.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "critical-path-calculator", |a: Args| {
            gizza_ai_critical_path_calculator_core::analyze(&a.tasks, &a.format)
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
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "tasks": { "type": "string", "description": "The task list, one task per line as `name, duration[, predecessor, ...]`. `duration` is a number, or a PERT three-point estimate `optimistic/most-likely/pessimistic` (e.g. `2/4/9`, reduced to (o+4m+p)/6). Predecessors are the names of tasks that must finish first. Example: `A, 3` then `B, 4, A` then `C, 2, A` then `D, 5, B, C`. Blank lines and lines starting with `#` are ignored." },
                    "format": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format: 'report' (a human-readable table of earliest/latest start & finish, total & free float, and the critical path) or 'json' (a machine-readable object with the same fields)." }
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
