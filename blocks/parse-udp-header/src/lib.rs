//! gizza-ai/parse-udp-header — decode a raw UDP datagram header (given as hex)
//! into source/destination ports (with well-known service names), total length,
//! implied payload length, and checksum. Chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_parse_udp_header_core::run as parse_header;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    header: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("header")
            .required()
            .describe("The UDP datagram header as a hex string (the 8-byte header; any trailing payload bytes are ignored). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. 'c3d20035 00281b6e'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseUdpHeader;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-udp-header",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a UDP datagram header from hex",
    skill(
        description = "Decode a raw UDP datagram header (RFC 768) given as a hex string into structured fields: the source and destination ports (each annotated with its well-known service name when recognised — DNS, NTP, DHCP, SNMP, QUIC, etc.), the total datagram length in bytes (the 8-byte header plus data), the implied payload length (length - 8), the 16-bit checksum as 0xNNNN, and a flag indicating whether the checksum is disabled (0x0000, allowed over IPv4). Only the first 8 bytes are read, so trailing payload bytes are ignored. Input may contain spaces, colons, dashes, dots, or a 0x prefix. Returns JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ParseUdpHeader {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-udp-header", |a: Args| {
            parse_header(&a.header).map_err(SkillError::InvalidArgs)
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
                    "header": { "type": "string", "description": "The UDP datagram header as a hex string (the 8-byte header; any trailing payload bytes are ignored). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. 'c3d20035 00281b6e'." }
                },
                "required": ["header"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
