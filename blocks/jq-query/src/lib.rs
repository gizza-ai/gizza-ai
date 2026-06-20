//! gizza-ai/jq-query — run a jq program against a JSON document (pure-Rust jaq).
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure (no_std jaq) → all backends incl. chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jq_query_core::run_jq;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    query: String,
    json: String,
    #[serde(default)]
    pretty: bool,
}

#[derive(Serialize)]
struct Resp {
    /// One serialized JSON value per jq output (a jq filter can yield 0, 1, or many).
    outputs: Vec<String>,
    count: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("query").required().describe("The jq program/filter, e.g. '.users | map(.name)'."))
        .param(Param::string("json").required().describe("The JSON document to run the filter against."))
        .param(Param::boolean("pretty").default(false).describe("Pretty-print (indent) each JSON output. Default false (compact)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JqQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jq-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run a jq program against JSON",
    skill(
        description = "Run a jq program/filter against a JSON document and return the transformed output, using a pure-Rust jq implementation (jaq) with the jq standard library (map, select, group_by, etc.). A filter can produce zero, one, or many values — each is returned as a serialized JSON string in `outputs`. Set pretty=true to indent. Example query: '.items | map(.price) | add'.",
        parameters = schema_json()
    )
)]
impl JqQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jq-query", |a: Args| {
            let outputs = run_jq(&a.query, &a.json, a.pretty).map_err(SkillError::InvalidArgs)?;
            let count = outputs.len();
            Ok(Resp { outputs, count })
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
                    "query":  { "type": "string", "description": "The jq program/filter, e.g. '.users | map(.name)'." },
                    "json":   { "type": "string", "description": "The JSON document to run the filter against." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print (indent) each JSON output. Default false (compact)." }
                },
                "required": ["query", "json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
