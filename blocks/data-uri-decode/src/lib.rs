//! gizza-ai/data-uri-decode — parse + decode a `data:` URI (RFC 2397) into its
//! MIME type, parameters (charset), encoding, and decoded payload.
//!
//! Thin chat-skill wrapper around `gizza-ai-data-uri-decode-core`. The chat
//! schema is single-sourced from `descriptor()` (shared across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    uri: String,
}

#[derive(Serialize)]
struct Resp {
    mime: String,
    params: Vec<[String; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    charset: Option<String>,
    encoding: String,
    bytes: usize,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detected_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hex_preview: Option<String>,
    truncated: bool,
}

impl From<gizza_ai_data_uri_decode_core::DataUri> for Resp {
    fn from(d: gizza_ai_data_uri_decode_core::DataUri) -> Self {
        Resp {
            mime: d.mime,
            params: d.params.into_iter().map(|(k, v)| [k, v]).collect(),
            charset: d.charset,
            encoding: d.encoding,
            bytes: d.bytes,
            kind: d.kind,
            text: d.text,
            detected_type: d.detected_type,
            hex_preview: d.hex_preview,
            truncated: d.truncated,
        }
    }
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("uri")
            .required()
            .describe("The data: URI to decode, e.g. 'data:text/plain;base64,SGVsbG8='."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DataUriDecode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-uri-decode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a data: URI into its MIME type and payload",
    skill(
        description = "Parse and decode a data: URI (RFC 2397) of the form data:[<mediatype>][;base64],<data>. Returns the MIME type (defaulting to text/plain when omitted), any media-type parameters such as charset, whether the payload was Base64 or percent-encoded, the decoded byte length, and the payload itself. A payload that decodes to printable UTF-8 is returned as text; a binary payload is reported with its detected file type (from magic bytes, e.g. image/png) and a hex preview. Tolerates whitespace/newlines inside Base64. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    )
)]
impl DataUriDecode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-uri-decode", |a: Args| {
            gizza_ai_data_uri_decode_core::decode(&a.uri)
                .map(Resp::from)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "uri": { "type": "string", "description": "The data: URI to decode, e.g. 'data:text/plain;base64,SGVsbG8='." }
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
