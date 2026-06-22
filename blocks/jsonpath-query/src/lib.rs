//! gizza-ai/jsonpath-query — evaluate a JSONPath (RFC 9535) expression against a JSON
//! document. Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure (serde_json_path) → all backends incl. chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jsonpath_query_core::query_jsonpath;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    path: String,
    json: String,
    #[serde(default)]
    pretty: bool,
    #[serde(default)]
    wrap: bool,
}

#[derive(Serialize)]
struct Resp {
    /// One serialized JSON value per matched node (a JSONPath query can select 0, 1,
    /// or many nodes). With wrap=true this is a single element: the JSON array of all
    /// matches (the canonical "result list").
    outputs: Vec<String>,
    count: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("path").required().describe("The JSONPath expression (RFC 9535), e.g. '$.store.book[*].author' or '$..price'."))
        .param(Param::string("json").required().describe("The JSON document to query."))
        .param(Param::boolean("pretty").default(false).describe("Pretty-print (indent) each emitted JSON value. Default false (compact)."))
        .param(Param::boolean("wrap").default(false).describe("Return a single JSON array of all matched values (the result list) instead of one value per match. Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonPathQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jsonpath-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Evaluate a JSONPath expression against JSON",
    skill(
        description = "Evaluate a JSONPath expression (RFC 9535) against a JSON document and return the selected nodes, using a pure-Rust RFC 9535 engine (serde_json_path). Supports the dot/bracket child access, wildcards ($.store.book[*]), array index/slice ([0], [1:3]), recursive descent ($..price), and filter selectors ($.book[?(@.price < 10)]). A query selects zero, one, or many nodes — each is returned as a serialized JSON string in `outputs`. Set wrap=true to return them as one JSON array; pretty=true to indent. Example: '$.store.book[?(@.price < 10)].title'.",
        parameters = schema_json()
    )
)]
impl JsonPathQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jsonpath-query", |a: Args| {
            let outputs = query_jsonpath(&a.path, &a.json, a.pretty, a.wrap).map_err(SkillError::InvalidArgs)?;
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
                    "path":   { "type": "string", "description": "The JSONPath expression (RFC 9535), e.g. '$.store.book[*].author' or '$..price'." },
                    "json":   { "type": "string", "description": "The JSON document to query." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print (indent) each emitted JSON value. Default false (compact)." },
                    "wrap":   { "type": "boolean", "default": false, "description": "Return a single JSON array of all matched values (the result list) instead of one value per match. Default false." }
                },
                "required": ["path", "json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
