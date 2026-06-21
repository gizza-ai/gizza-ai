//! gizza-ai/strip-ipv4-header — remove the IPv4 header from a raw packet (given
//! as hex), honoring the IHL field so any IP options are stripped too, and
//! return the encapsulated payload as hex. Chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_strip_ipv4_header_core::run as strip_header;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    packet: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("packet")
            .required()
            .describe("The raw IPv4 packet as a hex string (the IPv4 header followed by its payload). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. '4500001c1c46400040 11 0000 c0a80068 c0a80001 0035003500080000'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StripIpv4Header;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/strip-ipv4-header",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove the IPv4 header from a raw packet and return the payload",
    skill(
        description = "Remove the IPv4 header from a raw packet given as a hex string and return the encapsulated payload (the transport segment, e.g. the TCP/UDP datagram). The header length is read from the IHL field, so any IP options are stripped too; the packet's Total Length field is honored to drop trailing link-layer padding. Returns JSON with the IP version, IHL, the number of header bytes removed, the payload's protocol number (named when known, e.g. TCP/UDP/ICMP), the payload length, and the payload as a lowercase hex string. Input may contain spaces, colons, dashes, dots, or a 0x prefix. Runs locally.",
        parameters = schema_json()
    ),
)]
impl StripIpv4Header {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "strip-ipv4-header", |a: Args| {
            strip_header(&a.packet).map_err(SkillError::InvalidArgs)
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
                    "packet": { "type": "string", "description": "The raw IPv4 packet as a hex string (the IPv4 header followed by its payload). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. '4500001c1c46400040 11 0000 c0a80068 c0a80001 0035003500080000'." }
                },
                "required": ["packet"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
