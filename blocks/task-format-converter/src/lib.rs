//! gizza-ai/task-format-converter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_from")]
    from: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "default_pretty")]
    pretty: bool,
}

fn default_from() -> String {
    "auto".to_string()
}
fn default_to() -> String {
    "markdown".to_string()
}
fn default_pretty() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The task list to convert, in todo.txt, Markdown checklist, JSON, or CSV form."))
        .param(Param::enumv("from", ["auto", "todotxt", "markdown", "json", "csv"]).default("auto").describe("Source format. 'auto' sniffs it from the input (JSON braces, Markdown '- [ ]' items, a CSV header row, else todo.txt)."))
        .param(Param::enumv("to", ["todotxt", "markdown", "json", "csv"]).default("markdown").describe("Target format to convert the tasks into."))
        .param(Param::boolean("pretty").default(true).describe("Indent JSON output (ignored for non-JSON targets)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/task-format-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a task list between todo.txt, Markdown checklist, JSON, and CSV",
    skill(
        description = "Convert a task list between four formats in any direction: the todo.txt plain-text spec, a GitHub-flavored Markdown checklist ('- [ ] task'), a JSON array of task objects, and CSV with a fixed header. Every format parses into a common task model — description, completion state, priority, +projects, @contexts, creation/due/completion dates, and leftover key:value tags — so those attributes survive the round trip. Set 'from' to auto to sniff the source format from the input, and 'pretty' to indent JSON output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "task-format-converter", |a: Args| {
            gizza_ai_task_format_converter_core::run(&a.input, &a.from, &a.to, a.pretty)
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
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "The task list to convert, in todo.txt, Markdown checklist, JSON, or CSV form." },
                "from": { "type": "string", "enum": ["auto", "todotxt", "markdown", "json", "csv"], "default": "auto", "description": "Source format. 'auto' sniffs it from the input (JSON braces, Markdown '- [ ]' items, a CSV header row, else todo.txt)." },
                "to": { "type": "string", "enum": ["todotxt", "markdown", "json", "csv"], "default": "markdown", "description": "Target format to convert the tasks into." },
                "pretty": { "type": "boolean", "default": true, "description": "Indent JSON output (ignored for non-JSON targets)." }
            },
            "required": ["input"],
            "additionalProperties": false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
