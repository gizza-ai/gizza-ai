//! gizza-ai/action-item-extractor — deterministic action/decision extraction
//! from meeting notes. Chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → runs on
//! all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_action_item_extractor_core::extract;
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_group_by")]
    group_by: String,
    #[serde(default = "default_include_decisions")]
    include_decisions: bool,
}

fn default_format() -> String {
    "markdown".into()
}
fn default_group_by() -> String {
    "type".into()
}
fn default_include_decisions() -> bool {
    true
}

/// Single source for the chat schema (and CLI). Extraction is deterministic: the
/// tool only promotes explicit action markers, owner assignments, @handles,
/// imperative verbs, and decision markers; it never invents tasks.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Meeting notes, daily notes, or bullet lists to scan. Put one note or bullet per line. The extractor looks for explicit action markers (ACTION:, task:), owner assignments (Alice will...), @handles, imperative task verbs, and decision markers (Decided:, agreed, resolved)."),
        )
        .param(
            Param::enumv("format", ["markdown", "json"])
                .default("markdown")
                .describe("Output format. 'markdown' returns a checklist plus a Decisions section. 'json' returns {action_items:[{task,owner}],decisions:[...]} for automation."),
        )
        .param(
            Param::enumv("group_by", ["type", "owner"])
                .default("type")
                .describe("Markdown grouping. 'type' keeps one Action Items checklist with owners inline. 'owner' groups tasks under owner headings and puts unassigned tasks last."),
        )
        .param(
            Param::boolean("include_decisions")
                .default(true)
                .describe("Include a Decisions section (or decisions array in JSON) when decision markers are found. Turn off to return action items only."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/action-item-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract action items, owners, and decisions from notes",
    skill(
        description = "Turn meeting notes or daily notes into a structured action list. The extractor is deterministic: it pulls tasks from explicit signals such as ACTION/task markers, @handles, 'Alice will...' assignments, trailing owner tags, and imperative task verbs, and pulls decisions from markers like Decided, agreed, resolved, approved. It returns Markdown checklists or compact JSON with owners and decisions; it does not use an LLM or invent unstated tasks.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "action-item-extractor", |a: Args| {
            extract(&a.input, &a.format, &a.group_by, a.include_decisions)
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
                    "input": { "type": "string", "description": "Meeting notes, daily notes, or bullet lists to scan. Put one note or bullet per line. The extractor looks for explicit action markers (ACTION:, task:), owner assignments (Alice will...), @handles, imperative task verbs, and decision markers (Decided:, agreed, resolved)." },
                    "format": { "type": "string", "enum": ["markdown", "json"], "default": "markdown", "description": "Output format. 'markdown' returns a checklist plus a Decisions section. 'json' returns {action_items:[{task,owner}],decisions:[...]} for automation." },
                    "group_by": { "type": "string", "enum": ["type", "owner"], "default": "type", "description": "Markdown grouping. 'type' keeps one Action Items checklist with owners inline. 'owner' groups tasks under owner headings and puts unassigned tasks last." },
                    "include_decisions": { "type": "boolean", "default": true, "description": "Include a Decisions section (or decisions array in JSON) when decision markers are found. Turn off to return action items only." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
