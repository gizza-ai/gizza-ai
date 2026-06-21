//! gizza-ai/http-request-builder — build a raw HTTP/1.1 request message (no
//! request is sent). Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_http_request_builder_core::build;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    headers: String,
    #[serde(default)]
    body: String,
}

fn default_method() -> String {
    "GET".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("url").required().describe("The request URL (used for the request-target and Host header)."))
        .param(
            Param::enumv("method", ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"])
                .default("GET")
                .describe("HTTP method (default GET)."),
        )
        .param(
            Param::string("headers")
                .describe("Optional request headers, one 'Name: Value' per line. Host is added from the URL if omitted."),
        )
        .param(
            Param::string("body")
                .describe("Optional request body. A Content-Length header is added automatically when a body is given."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HttpRequestBuilder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/http-request-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a raw HTTP/1.1 request message",
    skill(
        description = "Build a well-formed raw HTTP/1.1 request message from a method, URL, headers, and body. NOTHING is sent — it just produces the text you'd put on the wire (e.g. for netcat/openssl s_client, teaching, or debugging). The request-target and Host come from the URL; headers are one 'Name: Value' per line; a Content-Length is added for a body. Lines are CRLF-terminated. To actually send a request, use the http-request tool.",
        parameters = schema_json()
    ),
)]
impl HttpRequestBuilder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "http-request-builder", |a: Args| {
            build(&a.method, &a.url, &a.headers, &a.body).map_err(SkillError::InvalidArgs)
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
                    "url":     { "type": "string", "description": "The request URL (used for the request-target and Host header)." },
                    "method":  { "type": "string", "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"], "default": "GET", "description": "HTTP method (default GET)." },
                    "headers": { "type": "string", "description": "Optional request headers, one 'Name: Value' per line. Host is added from the URL if omitted." },
                    "body":    { "type": "string", "description": "Optional request body. A Content-Length header is added automatically when a body is given." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
