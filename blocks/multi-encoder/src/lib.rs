//! gizza-ai/multi-encoder — encode/decode text across base64/hex/binary/url/
//! rot13/morse. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_multi_encoder_core::{parse_direction, parse_encoding, transform};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    encoding: String,
    #[serde(default)]
    direction: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text to encode or decode."))
        .param(Param::enumv("encoding", ["base64", "hex", "binary", "url", "rot13", "morse"]).required().describe("Which scheme: base64, hex, binary, url (percent-encoding), rot13, or morse."))
        .param(Param::enumv("direction", ["encode", "decode"]).default("encode").describe("encode (default) or decode. (rot13 is symmetric.)"))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MultiEncoder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/multi-encoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode/decode text (base64/hex/binary/url/rot13/morse)",
    skill(
        description = "Encode or decode text across multiple schemes in one tool: base64, hex, binary, url (percent-encoding), rot13, and morse code. Set encoding to the scheme and direction to encode (default) or decode (rot13 is symmetric). Binary/hex output is per-byte (space-separated for binary); morse uses space between letters and ' / ' between words.",
        parameters = schema_json()
    )
)]
impl MultiEncoder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "multi-encoder", |a: Args| {
            let enc = parse_encoding(&a.encoding).map_err(SkillError::InvalidArgs)?;
            let dir = parse_direction(&a.direction).map_err(SkillError::InvalidArgs)?;
            transform(&a.text, enc, dir).map_err(SkillError::InvalidArgs)
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
                    "text":      { "type": "string", "description": "The text to encode or decode." },
                    "encoding":  { "type": "string", "enum": ["base64", "hex", "binary", "url", "rot13", "morse"], "description": "Which scheme: base64, hex, binary, url (percent-encoding), rot13, or morse." },
                    "direction": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "encode (default) or decode. (rot13 is symmetric.)" }
                },
                "required": ["text", "encoding"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
