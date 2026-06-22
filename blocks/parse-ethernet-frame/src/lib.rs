//! gizza-ai/parse-ethernet-frame — decode an Ethernet II / IEEE 802.3 frame
//! (given as hex) into MAC addresses, EtherType / length, optional 802.1Q VLAN
//! tags, and the payload. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_parse_ethernet_frame_core::run as parse_frame;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    frame: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("frame")
            .required()
            .describe("The Ethernet frame as a hex string (without preamble/SFD/FCS). Spaces, colons, dashes, and a 0x prefix are ignored, e.g. 'ffffffffffff 001122334455 0806 ...'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseEthernetFrame;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-ethernet-frame",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode an Ethernet II / 802.3 frame from hex",
    skill(
        description = "Decode an Ethernet II or IEEE 802.3 frame given as a hex string into structured fields: destination and source MAC addresses (with broadcast/multicast and locally-administered flags), any 802.1Q / 802.1ad VLAN tags (TPID, PCP, DEI, VID — including Q-in-Q), the EtherType (named when known, e.g. IPv4/ARP/IPv6/LLDP) or the 802.3 length, and the payload length plus payload hex. Input may contain spaces, colons, dashes, or a 0x prefix. Returns JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ParseEthernetFrame {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-ethernet-frame", |a: Args| {
            parse_frame(&a.frame).map_err(SkillError::InvalidArgs)
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
                    "frame": { "type": "string", "description": "The Ethernet frame as a hex string (without preamble/SFD/FCS). Spaces, colons, dashes, and a 0x prefix are ignored, e.g. 'ffffffffffff 001122334455 0806 ...'." }
                },
                "required": ["frame"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
