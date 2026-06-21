//! gizza-ai/parse-tcp-header — decode a raw TCP segment header (given as hex)
//! into ports, sequence/ack numbers, data offset, control flags, window,
//! checksum, urgent pointer, and any TCP options. Chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_parse_tcp_header_core::run as parse_header;
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
            .describe("The TCP segment header as a hex string (at least the first 20 bytes). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. 'c21300504f8e6b1a000000005002fffffe340000'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseTcpHeader;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-tcp-header",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a TCP segment header from hex",
    skill(
        description = "Decode a raw TCP segment header given as a hex string into structured fields: source and destination ports, sequence and acknowledgement numbers (decimal and hex), data offset (header length in 32-bit words and bytes), the reserved bits, the control flags (NS, CWR, ECE, URG, ACK, PSH, RST, SYN, FIN) plus a space-separated list of the set flags, the receive window, the checksum, the urgent pointer, and any TCP options (named when known — MSS, Window Scale, SACK-Permitted, SACK, Timestamps — with their length and decoded value) when the data offset is greater than 5. Input may contain spaces, colons, dashes, dots, or a 0x prefix. Returns JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ParseTcpHeader {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-tcp-header", |a: Args| {
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
                    "header": { "type": "string", "description": "The TCP segment header as a hex string (at least the first 20 bytes). Spaces, colons, dashes, dots, and a 0x prefix are ignored, e.g. 'c21300504f8e6b1a000000005002fffffe340000'." }
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
