//! gizza-ai/json-to-typescript — infer TypeScript interfaces from a JSON sample.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_to_typescript_core::infer;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_root")]
    root_name: String,
    #[serde(default = "default_true")]
    export: bool,
}
fn default_root() -> String {
    "Root".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().describe("A JSON object or array sample to infer TypeScript types from."))
        .param(Param::string("root_name").default("Root").describe("Name for the top-level interface/type (PascalCased). Default 'Root'."))
        .param(Param::boolean("export").default(true).describe("Prefix declarations with 'export'. Default true."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonToTypescript;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-typescript",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer TypeScript interfaces from JSON",
    skill(
        description = "Infer TypeScript interface/type definitions from a pasted JSON object or array. Nested objects become their own interfaces; arrays of objects are merged so keys missing in some elements become optional (?) and differing types become unions (e.g. number | string). root_name names the top-level type (default 'Root'); export=true (default) prefixes 'export'. Returns the TypeScript source.",
        parameters = schema_json()
    )
)]
impl JsonToTypescript {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-typescript", |a: Args| {
            infer(&a.json, &a.root_name, a.export).map_err(SkillError::InvalidArgs)
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
                    "json":      { "type": "string", "description": "A JSON object or array sample to infer TypeScript types from." },
                    "root_name": { "type": "string", "default": "Root", "description": "Name for the top-level interface/type (PascalCased). Default 'Root'." },
                    "export":    { "type": "boolean", "default": true, "description": "Prefix declarations with 'export'. Default true." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
