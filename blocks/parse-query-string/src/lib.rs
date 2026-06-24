//! gizza-ai/parse-query-string — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    query: String,
    #[serde(default = "default_true")]
    plus_as_space: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("query")
                .required()
                .describe("The URL query string to parse, e.g. `name=John+Doe&color=red&color=blue&user[age]=30`. A leading `?` (or a full URL's `?...` part) is accepted and the `?` is stripped; surrounding whitespace is trimmed."),
        )
        .param(
            Param::boolean("plus_as_space")
                .default(true)
                .describe("When true (default), a `+` decodes to a space (application/x-www-form-urlencoded form semantics). Set false to keep `+` literal (RFC 3986 percent-encoding only)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-query-string",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a URL query string into structured key/value data",
    skill(
        description = "Parse a URL query string into JSON. Splits on `&` (and `;`), percent-decodes every key and value, and (by default) decodes `+` to a space. Returns: `count` (number of pairs); `pairs`, every `key`/`value` in source order with duplicates preserved (a bare key like `?flag` has no `value`, an empty `?k=` has `value` \"\"); and `structured`, an object where repeated keys collapse into arrays and PHP/Rails-style bracket notation is expanded — `a[]=1&a[]=2` becomes an array, `a[0]=x&a[2]=z` an indexed (null-padded) array, and `user[name]=Ann&user[age]=30` a nested object, including arrays of objects (`items[][id]=1`). Accepts a leading `?` or whole-URL query part. Set `plus_as_space` false to keep `+` literal. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-query-string", |a: Args| {
            gizza_ai_parse_query_string_core::run(&a.query, a.plus_as_space)
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
                    "query": { "type": "string", "description": "The URL query string to parse, e.g. `name=John+Doe&color=red&color=blue&user[age]=30`. A leading `?` (or a full URL's `?...` part) is accepted and the `?` is stripped; surrounding whitespace is trimmed." },
                    "plus_as_space": { "type": "boolean", "default": true, "description": "When true (default), a `+` decodes to a space (application/x-www-form-urlencoded form semantics). Set false to keep `+` literal (RFC 3986 percent-encoding only)." }
                },
                "required": ["query"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
