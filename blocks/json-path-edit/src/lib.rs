//! gizza-ai/json-path-edit — get, set, or delete a value at a dotted or bracketed
//! path in a JSON document. Thin chat-skill wrapper around
//! `gizza-ai-json-path-edit-core`. The chat schema is single-sourced from
//! `descriptor()` (shared shape across chat + CLI); the handler delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    path: String,
    #[serde(default)]
    operation: String,
    #[serde(default)]
    value: String,
    /// Pretty-print (indent) the output. Defaults to true when omitted.
    #[serde(default = "default_pretty")]
    pretty: bool,
}

fn default_pretty() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON document to read from or modify."),
        )
        .param(
            Param::string("path")
                .required()
                .describe("Dotted / bracketed path to the target value (lodash / dot-object style, NOT JSONPath). Examples: 'store.book[0].title', 'store.book.1.price' (dotted index), '[\"key with spaces\"].id', quote a key that itself contains a dot as '[\"a.b\"].c'. An optional leading '$' is ignored; an empty path selects the whole document."),
        )
        .param(
            Param::enumv("operation", ["get", "set", "delete"])
                .default("get")
                .describe("What to do at the path: 'get' (default) returns the value there; 'set' writes `value`, creating missing intermediate objects/arrays; 'delete' removes the key or array element. set/delete return the whole modified document."),
        )
        .param(
            Param::string("value")
                .default("")
                .describe("For operation='set', the value to write. Parsed as JSON (42 -> number, true -> boolean, null -> null, {\"k\":1} -> object); if it isn't valid JSON it's stored as a plain string, so wrap text in quotes to force a string. Ignored for get/delete."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print (indent) the output. Default true. 'get' returns the value at the path; 'set'/'delete' return the whole modified document."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonPathEdit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-path-edit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Get, set, or delete a value at a dotted/bracketed path in JSON.",
    skill(
        description = "Get, set, or delete a value at a dotted or bracketed path in a JSON document. The path uses lodash / dot-object notation (NOT RFC 9535 JSONPath): dot segments ('store.book.title'), array indices as brackets or dotted digits ('book[0]' == 'book.0'), and quoted keys for keys that contain a dot/bracket/space ('[\"a.b\"].c'). operation='get' (default) returns the value at the path; 'set' writes `value` (parsed as JSON, else a string) and creates any missing intermediate objects/arrays; 'delete' removes the key or array element. set/delete return the whole modified document; pretty=true (default) indents the output.",
        parameters = schema_json()
    ),
)]
impl JsonPathEdit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-path-edit", |a: Args| {
            gizza_ai_json_path_edit_core::edit(&a.json, &a.path, &a.operation, &a.value, a.pretty)
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
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON document to read from or modify." },
                    "path": { "type": "string", "description": "Dotted / bracketed path to the target value (lodash / dot-object style, NOT JSONPath). Examples: 'store.book[0].title', 'store.book.1.price' (dotted index), '[\"key with spaces\"].id', quote a key that itself contains a dot as '[\"a.b\"].c'. An optional leading '$' is ignored; an empty path selects the whole document." },
                    "operation": { "type": "string", "enum": ["get", "set", "delete"], "default": "get", "description": "What to do at the path: 'get' (default) returns the value there; 'set' writes `value`, creating missing intermediate objects/arrays; 'delete' removes the key or array element. set/delete return the whole modified document." },
                    "value": { "type": "string", "default": "", "description": "For operation='set', the value to write. Parsed as JSON (42 -> number, true -> boolean, null -> null, {\"k\":1} -> object); if it isn't valid JSON it's stored as a plain string, so wrap text in quotes to force a string. Ignored for get/delete." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print (indent) the output. Default true. 'get' returns the value at the path; 'set'/'delete' return the whole modified document." }
                },
                "required": ["json", "path"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
