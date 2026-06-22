//! gizza-ai/json-diff — structural diff of two JSON documents. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_diff_core::diff_documents;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("left")
                .required()
                .describe("The first (left/old) JSON document."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The second (right/new) JSON document to compare against the first."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Output indentation in spaces (1-8). Use 0 to minify. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonDiff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Structural diff of two JSON documents",
    skill(
        description = "Compare two JSON documents and report their structural differences. Objects are compared key-by-key and arrays index-by-index, recursively. Returns a JSON report: { equal, added, removed, changed, changes:[{ path, kind: added|removed|changed, old?, new? }] } where each path is a JSON path like $.a.b or $.list[2]. Output indentation is configurable (indent=0 minifies). Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonDiff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-diff", |a: Args| {
            diff_documents(&a.left, &a.right, a.indent as usize).map_err(SkillError::InvalidArgs)
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
                    "left":   { "type": "string", "description": "The first (left/old) JSON document." },
                    "right":  { "type": "string", "description": "The second (right/new) JSON document to compare against the first." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Output indentation in spaces (1-8). Use 0 to minify. Default 2." }
                },
                "required": ["left", "right"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
