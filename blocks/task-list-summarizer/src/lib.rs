//! gizza-ai/task-list-summarizer — summarizes Markdown task lists (GFM
//! checklists) into done/pending counts and filtered views.
//!
//! Thin chat-skill wrapper around `gizza-ai-task-list-summarizer-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mode: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The Markdown task list to summarize. Tasks are GFM checklist items like '- [ ] todo', '- [x] done', '* [X] done', or '1. [ ] todo'; non-checklist lines are ignored."),
        )
        .param(
            Param::enumv("mode", ["summary", "done", "pending", "json"])
                .default("summary")
                .describe("What to return. 'summary' (default) is a one-line count of total/done/pending and percent complete; 'done' lists the completed task texts; 'pending' lists the remaining task texts; 'json' returns a JSON object with the counts, percent, and both item lists."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/task-list-summarizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize a Markdown task list into done/pending counts and filtered views.",
    skill(
        description = "Summarize a Markdown task list (GFM checklist) into done/pending counts and filtered views. Pass the list as 'text'; tasks are checklist items like '- [ ] todo', '- [x] done', '* [X] done', or numbered '1. [ ] todo' (other lines are ignored). mode='summary' (default) returns a one-line count of total/done/pending and percent complete; mode='done' lists only the completed task texts (one per line); mode='pending' lists only the remaining task texts; mode='json' returns a JSON object {total, done, pending, percent, done_items, pending_items}.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "task-list-summarizer", |a: Args| {
            gizza_ai_task_list_summarizer_core::summarize(&a.text, &a.mode)
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
                    "text": { "type": "string", "description": "The Markdown task list to summarize. Tasks are GFM checklist items like '- [ ] todo', '- [x] done', '* [X] done', or '1. [ ] todo'; non-checklist lines are ignored." },
                    "mode": { "type": "string", "enum": ["summary", "done", "pending", "json"], "default": "summary", "description": "What to return. 'summary' (default) is a one-line count of total/done/pending and percent complete; 'done' lists the completed task texts; 'pending' lists the remaining task texts; 'json' returns a JSON object with the counts, percent, and both item lists." }
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
