//! gizza-ai/todo-organizer — turn a freeform brain-dump into a prioritized
//! Markdown checklist. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_todo_organizer_core::organize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_group")]
    group_by: String,
    #[serde(default)]
    numbered: bool,
    #[serde(default = "default_true")]
    show_due: bool,
}
fn default_group() -> String {
    "priority".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The freeform brain-dump, one task per line. Existing bullets/numbering are stripped."),
        )
        .param(
            Param::enumv("group_by", ["priority", "none"])
                .default("priority")
                .describe("priority (default) groups tasks under High/Medium/Low headings inferred from urgency keywords; none keeps the original order in one flat list."),
        )
        .param(
            Param::boolean("numbered")
                .default(false)
                .describe("When true, emit a numbered list (1. 2. 3.) instead of checkbox bullets (- [ ])."),
        )
        .param(
            Param::boolean("show_due")
                .default(true)
                .describe("When true (default), append any detected due-date hint (today, tomorrow, Friday, this week, …) as _(due: …)_."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TodoOrganizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/todo-organizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Organize a brain-dump into a prioritized Markdown checklist",
    skill(
        description = "Turn a freeform brain-dump (one task per line) into a prioritized Markdown checklist. Each task's priority is inferred from urgency keywords (urgent/asap/today → High, soon/this week → Medium, later/someday → Low; default Medium), and lightweight due-date hints (today, tomorrow, Friday, this week, …) are surfaced inline. By default tasks are grouped under High/Medium/Low headings (group_by=none keeps original order). Set numbered=true for a numbered list instead of checkboxes.",
        parameters = schema_json()
    )
)]
impl TodoOrganizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "todo-organizer", |a: Args| {
            organize(&a.text, &a.group_by, a.numbered, a.show_due).map_err(SkillError::InvalidArgs)
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
                    "text":     { "type": "string", "description": "The freeform brain-dump, one task per line. Existing bullets/numbering are stripped." },
                    "group_by": { "type": "string", "enum": ["priority", "none"], "default": "priority", "description": "priority (default) groups tasks under High/Medium/Low headings inferred from urgency keywords; none keeps the original order in one flat list." },
                    "numbered": { "type": "boolean", "default": false, "description": "When true, emit a numbered list (1. 2. 3.) instead of checkbox bullets (- [ ])." },
                    "show_due": { "type": "boolean", "default": true, "description": "When true (default), append any detected due-date hint (today, tomorrow, Friday, this week, …) as _(due: …)_." }
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
