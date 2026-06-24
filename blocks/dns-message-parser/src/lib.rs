//! gizza-ai/dns-message-parser — decode a DNS wire-format message (given as a hex
//! string or base64url) into the 12-byte header (id + flags: QR, opcode, AA, TC,
//! RD, RA, AD, CD, RCODE), the question section, and the answer/authority/
//! additional resource records — decompressing domain names and decoding the
//! common RR types (A, AAAA, NS, CNAME, PTR, MX, TXT, SOA, SRV, CAA, OPT/EDNS0).
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_dns_message_parser_core::run as parse_message;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("message")
            .required()
            .describe("The DNS message bytes as a hex string OR base64url (the format DNS-over-HTTPS uses in the GET ?dns= parameter), starting at the 12-byte header. For hex, spaces, colons, dashes, dots, commas, and a 0x prefix are ignored, e.g. a response begins '1234 8180 0001 0001 ...'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DnsMessageParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dns-message-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a DNS wire-format message from hex or base64url",
    skill(
        description = "Decode a DNS protocol message (RFC 1035 wire format) given as a hex string OR base64url into structured fields. Parses the 12-byte header — transaction id, QR (query/response), opcode (QUERY/IQUERY/STATUS/NOTIFY/UPDATE), the AA/TC/RD/RA/AD/CD flags, the RCODE (NOERROR/FORMERR/SERVFAIL/NXDOMAIN/REFUSED/…), and the QD/AN/NS/AR section counts — then the question section and the answer, authority, and additional resource records. Domain names are decompressed (the 0xC0 message-compression pointers are followed back to their target). Common record types are decoded to readable RDATA: A (IPv4), AAAA (IPv6, zero-run compressed), NS/CNAME/PTR/DNAME (a name), MX (preference + exchange), TXT/SPF (quoted strings), SOA (mname rname serial refresh retry expire minimum), SRV (priority weight port target), CAA (flags tag value), and OPT/EDNS0 (UDP payload size, version, DO bit); unknown types fall back to a hex dump. Accepts the base64url form used by DNS-over-HTTPS (the GET ?dns= parameter). Returns JSON. Runs locally; nothing is sent to a server.",
        parameters = schema_json()
    ),
)]
impl DnsMessageParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dns-message-parser", |a: Args| {
            parse_message(&a.message).map_err(SkillError::InvalidArgs)
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
                    "message": { "type": "string", "description": "The DNS message bytes as a hex string OR base64url (the format DNS-over-HTTPS uses in the GET ?dns= parameter), starting at the 12-byte header. For hex, spaces, colons, dashes, dots, commas, and a 0x prefix are ignored, e.g. a response begins '1234 8180 0001 0001 ...'." }
                },
                "required": ["message"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
