//! gizza-ai/parse-uri — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    uri: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("uri")
            .required()
            .describe("The URI or URL to split into components (e.g. https://user:pass@host:8443/path?a=1&b=2#frag). Absolute URIs and relative references are both accepted; surrounding whitespace is trimmed."),
    )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-uri",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a URI/URL into its RFC 3986 components",
    skill(
        description = "Parse a URI or URL into its RFC 3986 components and return JSON. Extracts: the scheme (lowercased); the authority and its parts — userinfo split into percent-decoded username/password, host (registered names lowercased, IPv6 kept in brackets), and numeric port; the origin (scheme://authority); the path (plus a percent-decoded view when it differs), the path split into decoded segments, and the filename with its extension; the raw query and the query parsed into ordered, percent-decoded key/value pairs with duplicates preserved (a '+' decodes to a space, ';' also separates pairs, a bare key has a null value); and the percent-decoded fragment. Handles relative references (no scheme), mailto:, file:// and IPv6 hosts. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-uri", |a: Args| {
            gizza_ai_parse_uri_core::run(&a.uri).map_err(SkillError::InvalidArgs)
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
                    "uri": { "type": "string", "description": "The URI or URL to split into components (e.g. https://user:pass@host:8443/path?a=1&b=2#frag). Absolute URIs and relative references are both accepted; surrounding whitespace is trimmed." }
                },
                "required": ["uri"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
