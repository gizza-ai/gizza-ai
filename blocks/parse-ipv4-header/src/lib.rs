//! gizza-ai/parse-ipv4-header — decode a raw IPv4 packet header (given as hex)
//! into version, IHL, DSCP/ECN, total length, identification, flags, fragment
//! offset, TTL, protocol, header checksum (validated), and addresses. Chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_parse_ipv4_header_core::run as parse_header;
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
            .describe("The IPv4 packet header as a hex string (at least the first 20 bytes). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. '4500003c1c46400040 06 b1e6 c0a80068 c0a80001'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseIpv4Header;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-ipv4-header",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode an IPv4 packet header from hex",
    skill(
        description = "Decode a raw IPv4 packet header given as a hex string into structured fields: version, IHL (header length), DSCP and ECN (from the ToS byte), total length and implied payload length, identification, flags (Don't Fragment / More Fragments), fragment offset (in 8-byte units and bytes), TTL, protocol number (named when known, e.g. TCP/UDP/ICMP/GRE/ESP), the header checksum (with a validity check computed over the header), source and destination IPv4 addresses, and any IP options when IHL > 5. Input may contain spaces, colons, dashes, dots, or a 0x prefix. Returns JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ParseIpv4Header {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-ipv4-header", |a: Args| {
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
                    "header": { "type": "string", "description": "The IPv4 packet header as a hex string (at least the first 20 bytes). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. '4500003c1c46400040 06 b1e6 c0a80068 c0a80001'." }
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
