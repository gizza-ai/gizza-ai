//! gizza-ai/parse-http-message — parse a raw HTTP/1.x request or response into
//! structured fields: the message kind, method/target (or status/reason), HTTP
//! version, headers (order + duplicates preserved), and the body. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_parse_http_message_core::run as parse_message;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("message")
            .required()
            .describe("The raw HTTP/1.x request or response to parse, including the start line, headers, a blank line, and any body. Request and response messages are detected automatically. Bare-LF line endings are tolerated."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseHttpMessage;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-http-message",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a raw HTTP/1.x request or response into structured fields",
    skill(
        description = "Parse a raw HTTP/1.x request or response message into structured JSON. Detects whether the message is a request or a response, then extracts: the method, request target, path, and query (for requests) or the status code, reason phrase, and status class (for responses); the HTTP version; every header in wire order with duplicates preserved (e.g. multiple Set-Cookie lines); convenience fields for Content-Type, Content-Length, and chunked Transfer-Encoding; and the body with its byte length. Handles CRLF or bare-LF line endings and obsolete header line folding. Returns JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ParseHttpMessage {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-http-message", |a: Args| {
            parse_message(&a.message).map_err(SkillError::InvalidArgs)
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
                    "message": { "type": "string", "description": "The raw HTTP/1.x request or response to parse, including the start line, headers, a blank line, and any body. Request and response messages are detected automatically. Bare-LF line endings are tolerated." }
                },
                "required": ["message"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
