//! gizza-ai/json-beautify — pretty-print or minify JSON with configurable
//! indentation, validating it. Chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_beautify_core::format;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().describe("The JSON text to pretty-print or minify."))
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonBeautify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-beautify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pretty-print or minify (and validate) JSON",
    skill(
        description = "Pretty-print messy or minified JSON with configurable indentation, validating it in the process (returns a line/column error if invalid). indent is spaces per level (1-8, default 2); set indent=0 to minify to a single compact line instead. Object key order is preserved. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonBeautify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-beautify", |a: Args| {
            format(&a.json, a.indent as usize).map_err(SkillError::InvalidArgs)
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
                    "json":   { "type": "string", "description": "The JSON text to pretty-print or minify." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2." }
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
