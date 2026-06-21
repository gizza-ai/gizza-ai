//! gizza-ai/extract-decode-base64 — scan text for embedded Base64 blobs and
//! decode them (text, or file-type + hex for binaries). Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_decode_base64_core::extract;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
}

#[derive(Serialize)]
struct Resp {
    count: usize,
    blobs: Vec<gizza_ai_extract_decode_base64_core::Decoded>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("text")
            .required()
            .describe("The text to scan for embedded Base64 blobs."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractDecodeBase64;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-decode-base64",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and decode Base64 blobs embedded in text",
    skill(
        description = "Scan a block of text for embedded Base64 blobs (standard or URL-safe), decode each, and report it. Decoded UTF-8 text is returned as text; recognized binaries are reported with their detected file type (e.g. image/png) and a hex preview. Random alphanumeric noise that doesn't decode to printable text or a known file type is ignored. Returns each blob's source, kind (text/binary), byte length, and decoded content. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractDecodeBase64 {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-decode-base64", |a: Args| {
            let blobs = extract(&a.text);
            Ok::<Resp, SkillError>(Resp { count: blobs.len(), blobs })
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
                    "text": { "type": "string", "description": "The text to scan for embedded Base64 blobs." }
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
