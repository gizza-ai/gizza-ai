//! gizza-ai/harmony-format-renderer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    messages: String,
    #[serde(default = "d_input_format")]
    input_format: String,
    #[serde(default)]
    instructions: String,
    #[serde(default)]
    tools: String,
    #[serde(default)]
    model_identity: String,
    #[serde(default = "d_effort")]
    reasoning_effort: String,
    #[serde(default = "d_cutoff")]
    knowledge_cutoff: String,
    #[serde(default)]
    current_date: String,
    #[serde(default = "d_true")]
    include_system: bool,
    #[serde(default = "d_render_target")]
    render_target: String,
    #[serde(default = "d_true")]
    auto_drop_analysis: bool,
    #[serde(default = "d_output_format")]
    output_format: String,
}

fn d_input_format() -> String {
    "auto".to_string()
}
fn d_effort() -> String {
    "medium".to_string()
}
fn d_cutoff() -> String {
    "2024-06".to_string()
}
fn d_render_target() -> String {
    "completion".to_string()
}
fn d_output_format() -> String {
    "text".to_string()
}
fn d_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("messages").required().describe(
            "The conversation to render. Either a JSON array of message objects — \
             [{\"role\":\"user\",\"content\":\"hi\"}] with optional \"channel\" \
             (analysis|commentary|final), \"recipient\" (the function an assistant turn calls) \
             and \"name\" (the function a tool turn came from) — or one turn per line as \
             `role: content`, e.g. `assistant[analysis]: thinking…` and \
             `assistant[commentary] to=get_weather: {\"city\":\"Oslo\"}`. Roles: system, \
             developer, user, assistant, tool. Max 200000 characters and 500 turns.",
        ))
        .param(
            Param::enumv("input_format", ["auto", "json", "lines"])
                .default("auto")
                .describe(
                    "How to read `messages`: 'auto' (JSON if it starts with [ or {, else lines), \
                     'json' (force a JSON array), or 'lines' (force the `role: content` form).",
                ),
        )
        .param(Param::string("instructions").default("").describe(
            "Developer instructions — the text that would be an OpenAI 'system prompt'. Rendered \
             under '# Instructions' in the developer message. Any system/developer turn found in \
             `messages` is appended after this. Leave empty to omit the section.",
        ))
        .param(Param::string("tools").default("").describe(
            "Function tools as JSON: an array of {\"name\":…,\"description\":…,\"parameters\":{JSON \
             Schema}}. The Chat Completions wrapper {\"type\":\"function\",\"function\":{…}} is \
             also accepted. Rendered as the 'namespace functions { … }' TypeScript-style block and \
             adds the commentary-channel clause to the system message. Max 50000 characters.",
        ))
        .param(Param::string("model_identity").default("").describe(
            "First line of the system message. Blank uses the gpt-oss default, \
             'You are ChatGPT, a large language model trained by OpenAI.'",
        ))
        .param(
            Param::enumv("reasoning_effort", ["low", "medium", "high", "none"])
                .default("medium")
                .describe(
                    "Value for the system message's 'Reasoning:' line — 'low', 'medium' or \
                     'high'. Use 'none' to omit the line entirely.",
                ),
        )
        .param(Param::string("knowledge_cutoff").default("2024-06").describe(
            "Value for the 'Knowledge cutoff:' line, normally YYYY-MM. Defaults to 2024-06, the \
             gpt-oss default.",
        ))
        .param(Param::string("current_date").default("").describe(
            "Value for the 'Current date:' line, normally YYYY-MM-DD (e.g. 2025-06-28). Leave \
             empty to omit the line.",
        ))
        .param(Param::boolean("include_system").default(true).describe(
            "Emit the system (metadata) message. Turn off to render only the developer and \
             conversation turns — useful when appending to an existing prompt.",
        ))
        .param(
            Param::enumv("render_target", ["completion", "conversation"])
                .default("completion")
                .describe(
                    "'completion' appends the '<|start|>assistant' generation prompt so the \
                     result is ready to sample from; 'conversation' renders the turns only.",
                ),
        )
        .param(Param::boolean("auto_drop_analysis").default(true).describe(
            "Apply Harmony's chain-of-thought rule: drop assistant 'analysis' turns that come \
             before the last 'final' answer, keeping analysis from the in-flight tool-calling \
             chain. Turn off to render every turn verbatim.",
        ))
        .param(
            Param::enumv("output_format", ["text", "json"])
                .default("text")
                .describe(
                    "'text' returns the rendered prompt; 'json' returns a report object with the \
                     prompt plus message_count, rendered_message_count, dropped_analysis_count, \
                     tool_count, char_count and the stop tokens.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/harmony-format-renderer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a conversation into OpenAI's Harmony response format for gpt-oss",
    skill(
        description = "Render a system/developer/user/assistant/tool conversation into the Harmony response format that the gpt-oss open-weight models are trained on — the flat token string built from <|start|>, <|channel|>, <|constrain|>, <|message|>, <|end|> and <|call|>. Pass `messages` as a JSON array of {role, content, channel?, recipient?, name?} objects or as `role: content` lines. It builds the system metadata message (model identity, 'Knowledge cutoff:', optional 'Current date:', 'Reasoning: low|medium|high', and the valid-channels line), folds any system/developer turn plus the `instructions` param into the developer message's '# Instructions' section, renders `tools` (JSON Schema function definitions) as the 'namespace functions { … }' block and adds the commentary-channel clause, emits assistant turns on the analysis/commentary/final channels, renders tool calls as 'to=functions.NAME <|constrain|>json … <|call|>' and tool results as 'functions.NAME to=assistant', applies Harmony's drop-the-old-chain-of-thought rule via auto_drop_analysis, and appends the '<|start|>assistant' generation prompt when render_target is 'completion'. Set output_format to 'json' for a report with the prompt plus message and character counts. Fully deterministic string assembly — no tokenizer, no LLM, no network; it does not emit token IDs.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "harmony-format-renderer", |a: Args| {
            gizza_ai_harmony_format_renderer_core::run(
                &a.messages,
                &a.input_format,
                &a.instructions,
                &a.tools,
                &a.model_identity,
                &a.reasoning_effort,
                &a.knowledge_cutoff,
                &a.current_date,
                a.include_system,
                &a.render_target,
                a.auto_drop_analysis,
                &a.output_format,
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
            "type": "object",
            "properties": {
                "messages": { "type": "string", "description": "The conversation to render. Either a JSON array of message objects — [{\"role\":\"user\",\"content\":\"hi\"}] with optional \"channel\" (analysis|commentary|final), \"recipient\" (the function an assistant turn calls) and \"name\" (the function a tool turn came from) — or one turn per line as `role: content`, e.g. `assistant[analysis]: thinking…` and `assistant[commentary] to=get_weather: {\"city\":\"Oslo\"}`. Roles: system, developer, user, assistant, tool. Max 200000 characters and 500 turns." },
                "input_format": { "type": "string", "enum": ["auto", "json", "lines"], "default": "auto", "description": "How to read `messages`: 'auto' (JSON if it starts with [ or {, else lines), 'json' (force a JSON array), or 'lines' (force the `role: content` form)." },
                "instructions": { "type": "string", "default": "", "description": "Developer instructions — the text that would be an OpenAI 'system prompt'. Rendered under '# Instructions' in the developer message. Any system/developer turn found in `messages` is appended after this. Leave empty to omit the section." },
                "tools": { "type": "string", "default": "", "description": "Function tools as JSON: an array of {\"name\":…,\"description\":…,\"parameters\":{JSON Schema}}. The Chat Completions wrapper {\"type\":\"function\",\"function\":{…}} is also accepted. Rendered as the 'namespace functions { … }' TypeScript-style block and adds the commentary-channel clause to the system message. Max 50000 characters." },
                "model_identity": { "type": "string", "default": "", "description": "First line of the system message. Blank uses the gpt-oss default, 'You are ChatGPT, a large language model trained by OpenAI.'" },
                "reasoning_effort": { "type": "string", "enum": ["low", "medium", "high", "none"], "default": "medium", "description": "Value for the system message's 'Reasoning:' line — 'low', 'medium' or 'high'. Use 'none' to omit the line entirely." },
                "knowledge_cutoff": { "type": "string", "default": "2024-06", "description": "Value for the 'Knowledge cutoff:' line, normally YYYY-MM. Defaults to 2024-06, the gpt-oss default." },
                "current_date": { "type": "string", "default": "", "description": "Value for the 'Current date:' line, normally YYYY-MM-DD (e.g. 2025-06-28). Leave empty to omit the line." },
                "include_system": { "type": "boolean", "default": true, "description": "Emit the system (metadata) message. Turn off to render only the developer and conversation turns — useful when appending to an existing prompt." },
                "render_target": { "type": "string", "enum": ["completion", "conversation"], "default": "completion", "description": "'completion' appends the '<|start|>assistant' generation prompt so the result is ready to sample from; 'conversation' renders the turns only." },
                "auto_drop_analysis": { "type": "boolean", "default": true, "description": "Apply Harmony's chain-of-thought rule: drop assistant 'analysis' turns that come before the last 'final' answer, keeping analysis from the in-flight tool-calling chain. Turn off to render every turn verbatim." },
                "output_format": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "'text' returns the rendered prompt; 'json' returns a report object with the prompt plus message_count, rendered_message_count, dropped_analysis_count, tool_count, char_count and the stop tokens." }
            },
            "required": ["messages"],
            "additionalProperties": false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
