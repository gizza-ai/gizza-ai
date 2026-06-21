//! gizza-ai/strip-udp-header — remove the fixed 8-byte UDP header from a raw UDP
//! datagram (given as hex) and return the encapsulated application payload as
//! hex, plus the decoded header fields. Chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_strip_udp_header_core::run as strip_header;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    datagram: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("datagram")
            .required()
            .describe("The raw UDP datagram as a hex string (the 8-byte UDP header followed by its payload). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. '0035 a1b2 000c 1234 deadbeef'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StripUdpHeader;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/strip-udp-header",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove the 8-byte UDP header from a raw datagram and return the payload",
    skill(
        description = "Remove the fixed 8-byte UDP header from a raw UDP datagram given as a hex string and return the encapsulated application payload (the bytes the UDP layer carries). Returns JSON with the source and destination ports, the UDP Length and Checksum fields, the number of header bytes removed (always 8), the payload length, and the payload as a lowercase hex string. The UDP Length field is honored to drop any trailing link-layer padding when it is consistent. Input may contain spaces, colons, dashes, dots, or a 0x prefix. Runs locally.",
        parameters = schema_json()
    ),
)]
impl StripUdpHeader {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "strip-udp-header", |a: Args| {
            strip_header(&a.datagram).map_err(SkillError::InvalidArgs)
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
                    "datagram": { "type": "string", "description": "The raw UDP datagram as a hex string (the 8-byte UDP header followed by its payload). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. '0035 a1b2 000c 1234 deadbeef'." }
                },
                "required": ["datagram"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
