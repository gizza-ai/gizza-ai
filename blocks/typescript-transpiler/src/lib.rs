//! gizza-ai/typescript-transpiler — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. It performs a local,
//! best-effort TypeScript syntax strip for snippets and small files.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    enum_style: String,
    #[serde(default)]
    remove_comments: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("TypeScript source to convert into best-effort JavaScript by removing type-only syntax. Maximum 1 MB."))
        .param(Param::enumv("enum_style", ["compile", "strip"]).default("compile").describe("How to handle TypeScript enum declarations: compile them to simple JavaScript objects, or strip them as type-only declarations."))
        .param(Param::boolean("remove_comments").default(false).describe("Remove line and block comments from the output in addition to stripping TypeScript syntax."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/typescript-transpiler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip TypeScript syntax into best-effort JavaScript.",
    skill(
        description = "Transpile small TypeScript snippets to JavaScript locally by stripping type annotations, interfaces, type aliases, assertions, optional markers, modifiers, implements clauses, type-only imports/exports, and simple enum declarations. This is a deterministic syntax stripper, not a full TypeScript compiler or type checker.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "typescript-transpiler", |a: Args| {
            let enum_style = gizza_ai_typescript_transpiler_core::parse_enum_style(&a.enum_style)
                .map_err(SkillError::InvalidArgs)?;
            let options = gizza_ai_typescript_transpiler_core::Options {
                enum_style,
                remove_comments: a.remove_comments,
            };
            gizza_ai_typescript_transpiler_core::transpile(&a.input, &options)
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
              "type":"object",
              "properties":{
                "input":{"type":"string","description":"TypeScript source to convert into best-effort JavaScript by removing type-only syntax. Maximum 1 MB."},
                "enum_style":{"type":"string","enum":["compile","strip"],"default":"compile","description":"How to handle TypeScript enum declarations: compile them to simple JavaScript objects, or strip them as type-only declarations."},
                "remove_comments":{"type":"boolean","default":false,"description":"Remove line and block comments from the output in addition to stripping TypeScript syntax."}
              },
              "required":["input"],
              "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
