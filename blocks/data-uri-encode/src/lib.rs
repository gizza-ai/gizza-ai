//! gizza-ai/data-uri-encode — build a `data:` URI (RFC 2397) from text, with a
//! chosen MIME type and Base64 or percent (URL) encoding.
//!
//! Thin chat-skill wrapper around `gizza-ai-data-uri-encode-core`. The chat
//! schema is single-sourced from `descriptor()` (shared with the CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mime: String,
    #[serde(default)]
    encoding: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text content to embed in the data: URI."),
        )
        .param(
            Param::string("mime")
                .default("text/plain")
                .describe("Media type to embed, e.g. 'text/plain', 'text/html', 'image/svg+xml', or 'application/json'. May include parameters like 'text/html;charset=utf-8'. Default 'text/plain'."),
        )
        .param(
            Param::enumv("encoding", ["base64", "url"])
                .default("base64")
                .describe("Payload encoding. 'base64' (default) emits 'data:<mime>;base64,<...>' — compact and works for any content; 'url' percent-encodes the text into 'data:<mime>,<...>' — readable for short ASCII text."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-uri-encode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a data: URI from text",
    skill(
        description = "Build a self-contained 'data:' URI (RFC 2397) from text, for inline embedding in CSS url(...), an <img src>, a JSON payload, or an email. Pass the content as 'text', the media type as 'mime' (e.g. 'text/plain', 'text/html', 'image/svg+xml', 'application/json'; default 'text/plain', and a 'charset' parameter is allowed), and 'encoding' as 'base64' (default — 'data:<mime>;base64,<...>') or 'url' (percent-encoded — 'data:<mime>,<...>'). Returns the data: URI plus its mime, encoding, input byte count, and URI length. To go the other way, use data-uri-decode; to encode a file, use file-to-data-uri.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-uri-encode", |a: Args| {
            gizza_ai_data_uri_encode_core::run(&a.text, &a.mime, &a.encoding)
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text content to embed in the data: URI." },
                    "mime": { "type": "string", "default": "text/plain", "description": "Media type to embed, e.g. 'text/plain', 'text/html', 'image/svg+xml', or 'application/json'. May include parameters like 'text/html;charset=utf-8'. Default 'text/plain'." },
                    "encoding": { "type": "string", "enum": ["base64", "url"], "default": "base64", "description": "Payload encoding. 'base64' (default) emits 'data:<mime>;base64,<...>' — compact and works for any content; 'url' percent-encodes the text into 'data:<mime>,<...>' — readable for short ASCII text." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
