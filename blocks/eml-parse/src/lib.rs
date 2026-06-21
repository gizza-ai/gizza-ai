//! gizza-ai/eml-parse — parse a raw .eml / RFC 5322 message into structured
//! headers, decoded text/HTML bodies, and an attachment list. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_eml_parse_core::parse;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    eml: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("eml")
            .required()
            .describe("The raw .eml / RFC 822 message text (headers plus body, including MIME parts)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EmlParse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/eml-parse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a raw .eml / RFC 822 email message",
    skill(
        description = "Parse a raw .eml / RFC 5322 email message into structured fields: subject, date, message-id, from/to/cc (name + address), the decoded plain-text and HTML bodies, and a list of attachments (filename, MIME content-type, size in bytes). MIME multipart messages and encoded headers/bodies are decoded automatically. Runs locally.",
        parameters = schema_json()
    ),
)]
impl EmlParse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "eml-parse", |a: Args| {
            parse(&a.eml).map_err(SkillError::InvalidArgs)
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
                    "eml": { "type": "string", "description": "The raw .eml / RFC 822 message text (headers plus body, including MIME parts)." }
                },
                "required": ["eml"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
