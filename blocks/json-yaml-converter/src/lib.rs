//! gizza-ai/json-yaml-converter — convert between JSON and YAML. Thin wrapper
//! around the core; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_yaml_converter_core::{convert, resolve_direction};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    pretty: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The JSON or YAML text to convert."))
        .param(Param::enumv("direction", ["auto", "json-to-yaml", "yaml-to-json"]).default("auto").describe("Conversion direction. 'auto' (default): input starting with '{' or '[' is treated as JSON (-> YAML), otherwise YAML (-> JSON)."))
        .param(Param::boolean("pretty").default(false).describe("Pretty-print (indent) JSON output. Only affects yaml-to-json. Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonYamlConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-yaml-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between JSON and YAML",
    skill(
        description = "Convert between JSON and YAML in either direction. direction='auto' (default) detects the input (text starting with '{' or '[' is JSON -> YAML, otherwise YAML -> JSON); set 'json-to-yaml' or 'yaml-to-json' to force it. pretty=true indents JSON output (yaml-to-json only). Data is preserved round-trip.",
        parameters = schema_json()
    )
)]
impl JsonYamlConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-yaml-converter", |a: Args| {
            let dir = resolve_direction(&a.direction, &a.input).map_err(SkillError::InvalidArgs)?;
            convert(&a.input, dir, a.pretty).map_err(SkillError::InvalidArgs)
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
                    "input":     { "type": "string", "description": "The JSON or YAML text to convert." },
                    "direction": { "type": "string", "enum": ["auto", "json-to-yaml", "yaml-to-json"], "default": "auto", "description": "Conversion direction. 'auto' (default): input starting with '{' or '[' is treated as JSON (-> YAML), otherwise YAML (-> JSON)." },
                    "pretty":    { "type": "boolean", "default": false, "description": "Pretty-print (indent) JSON output. Only affects yaml-to-json. Default false." }
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
