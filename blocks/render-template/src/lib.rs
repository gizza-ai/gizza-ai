//! gizza-ai/render-template — chat skill block on the shared tool abstraction.
//! Renders a Handlebars/Mustache template against supplied JSON data. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure — runs entirely inside
//! the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    template: String,
    #[serde(default)]
    data: String,
    #[serde(default = "default_engine")]
    engine: String,
    #[serde(default)]
    strict: bool,
}

fn default_engine() -> String {
    "handlebars".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("template")
                .required()
                .describe("The Handlebars/Mustache template, e.g. \"Hello {{name}}!\" or \"{{#each items}}- {{this}}\\n{{/each}}\"."),
        )
        .param(
            Param::string("data")
                .describe("JSON object (or array) supplying the template variables, e.g. {\"name\":\"Ada\"}. Defaults to an empty object."),
        )
        .param(
            Param::enumv("engine", ["handlebars", "mustache"])
                .default("handlebars")
                .describe("Template dialect. Handlebars is a superset of Mustache; both support {{var}}, nested paths, {{#each}} and {{#if}}."),
        )
        .param(
            Param::boolean("strict")
                .default(false)
                .describe("When true, referencing a missing variable is an error; when false (default) it renders as empty."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/render-template",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a Handlebars/Mustache template against JSON data",
    skill(
        description = "Render a Handlebars/Mustache template against supplied JSON data and return the output text. Supports {{variable}} substitution, nested paths ({{user.email}}), loops ({{#each}}), and conditionals ({{#if}}). Useful for generating emails, config files, code, or any text from a template + data.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "render-template", |a: Args| {
            gizza_ai_render_template_core::render(&a.template, &a.data, &a.engine, a.strict)
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
                    "template": { "type": "string", "description": "The Handlebars/Mustache template, e.g. \"Hello {{name}}!\" or \"{{#each items}}- {{this}}\\n{{/each}}\"." },
                    "data": { "type": "string", "description": "JSON object (or array) supplying the template variables, e.g. {\"name\":\"Ada\"}. Defaults to an empty object." },
                    "engine": { "type": "string", "enum": ["handlebars", "mustache"], "default": "handlebars", "description": "Template dialect. Handlebars is a superset of Mustache; both support {{var}}, nested paths, {{#each}} and {{#if}}." },
                    "strict": { "type": "boolean", "default": false, "description": "When true, referencing a missing variable is an error; when false (default) it renders as empty." }
                },
                "required": ["template"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
