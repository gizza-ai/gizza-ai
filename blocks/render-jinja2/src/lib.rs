//! gizza-ai/render-jinja2 — chat skill block on the shared tool abstraction.
//! Renders a Jinja2 (Jinja) template against supplied JSON or YAML data. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure — runs entirely inside the
//! WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    template: String,
    #[serde(default)]
    data: String,
    #[serde(default = "default_format")]
    data_format: String,
    #[serde(default)]
    strict: bool,
}

fn default_format() -> String {
    "auto".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("template")
                .required()
                .describe("The Jinja2 template, e.g. \"Hello {{ name }}!\" or \"{% for i in items %}- {{ i }}\\n{% endfor %}\"."),
        )
        .param(
            Param::string("data")
                .describe("JSON or YAML supplying the template variables, e.g. {\"name\":\"Ada\"}. Defaults to an empty object."),
        )
        .param(
            Param::enumv("data_format", ["auto", "json", "yaml"])
                .default("auto")
                .describe("Format of the data: 'auto' detects JSON or YAML, or force 'json'/'yaml'."),
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
    name = "gizza-ai/render-jinja2",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a Jinja2 template against JSON or YAML data",
    skill(
        description = "Render a Jinja2 (Jinja) template against supplied JSON or YAML data and return the output text. Supports {{ variable }} substitution, nested paths ({{ user.email }}), {% for %} loops, {% if %}/{% elif %}/{% else %} conditionals, and filters ({{ name | upper }}, {{ list | join(', ') }}). Set data_format to 'json', 'yaml', or 'auto' (default), and strict to error on a missing variable instead of rendering it empty. Useful for generating emails, config files, code, or any text from a template + data. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "render-jinja2", |a: Args| {
            gizza_ai_render_jinja2_core::render(&a.template, &a.data, &a.data_format, a.strict)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "template": { "type": "string", "description": "The Jinja2 template, e.g. \"Hello {{ name }}!\" or \"{% for i in items %}- {{ i }}\\n{% endfor %}\"." },
                    "data": { "type": "string", "description": "JSON or YAML supplying the template variables, e.g. {\"name\":\"Ada\"}. Defaults to an empty object." },
                    "data_format": { "type": "string", "enum": ["auto", "json", "yaml"], "default": "auto", "description": "Format of the data: 'auto' detects JSON or YAML, or force 'json'/'yaml'." },
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
