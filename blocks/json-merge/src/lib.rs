//! gizza-ai/json-merge — deep-merge two or more JSON documents. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_merge_core::merge_documents;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    documents: String,
    #[serde(default)]
    concat_arrays: bool,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("documents").required().describe(
            "Two or more JSON values to merge, concatenated (whitespace/newline separated), e.g. '{\"a\":1}\\n{\"b\":2}'. Merged left-to-right.",
        ))
        .param(
            Param::boolean("concat_arrays")
                .default(false)
                .describe("When arrays collide: false (default) = the later array replaces the earlier; true = concatenate them."),
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
struct JsonMerge;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Deep-merge two or more JSON documents",
    skill(
        description = "Deep-merge two or more JSON documents (provided concatenated, whitespace/newline separated) left-to-right. Objects merge recursively; on a scalar/type conflict the later document wins. Arrays are replaced by default, or concatenated when concat_arrays=true. Output indentation is configurable (indent=0 minifies). Returns the merged JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonMerge {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-merge", |a: Args| {
            merge_documents(&a.documents, a.concat_arrays, a.indent as usize)
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
                "type": "object",
                "properties": {
                    "documents":     { "type": "string", "description": "Two or more JSON values to merge, concatenated (whitespace/newline separated), e.g. '{\"a\":1}\\n{\"b\":2}'. Merged left-to-right." },
                    "concat_arrays": { "type": "boolean", "default": false, "description": "When arrays collide: false (default) = the later array replaces the earlier; true = concatenate them." },
                    "indent":        { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Output indentation in spaces (1-8). Use 0 to minify. Default 2." }
                },
                "required": ["documents"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
