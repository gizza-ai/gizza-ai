//! gizza-ai/json-yaml-convert — convert config data between JSON, YAML, and TOML.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_yaml_convert_core::{convert, Format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    from: String,
    to: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The config data to convert."))
        .param(
            Param::enumv("from", ["json", "yaml", "toml"])
                .required()
                .describe("Source format of the input."),
        )
        .param(
            Param::enumv("to", ["json", "yaml", "toml"])
                .required()
                .describe("Target format to convert to."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the output (JSON/TOML). Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonYamlConvert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-yaml-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between JSON, YAML, and TOML",
    skill(
        description = "Convert config data between JSON, YAML, and TOML in any direction. Specify from and to (json/yaml/toml); pretty indents JSON/TOML output (default true). Note: TOML output requires a top-level object and has no null. Round-trips data through a common model. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonYamlConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-yaml-convert", |a: Args| {
            let from = Format::parse(&a.from).map_err(SkillError::InvalidArgs)?;
            let to = Format::parse(&a.to).map_err(SkillError::InvalidArgs)?;
            convert(&a.input, from, to, a.pretty).map_err(SkillError::InvalidArgs)
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
                    "input":  { "type": "string", "description": "The config data to convert." },
                    "from":   { "type": "string", "enum": ["json", "yaml", "toml"], "description": "Source format of the input." },
                    "to":     { "type": "string", "enum": ["json", "yaml", "toml"], "description": "Target format to convert to." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent the output (JSON/TOML). Default true." }
                },
                "required": ["input", "from", "to"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
