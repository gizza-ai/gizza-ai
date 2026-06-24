//! gizza-ai/ja3-fingerprint — compute the JA3 TLS client fingerprint string and
//! its MD5 hash from a raw ClientHello given as a hex string. Chat schema is
//! single-sourced from descriptor(); handle() delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ja3_fingerprint_core::run as compute_ja3;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    client_hello: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("client_hello")
            .required()
            .describe("The TLS ClientHello as a hex string. It may start at the 5-byte TLS record header (begins '16 03 ...'), the handshake header (begins '01 ...'), or directly at the ClientHello body. Spaces, colons, dashes, dots, commas, and a 0x prefix are ignored."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Ja3Fingerprint;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ja3-fingerprint",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the JA3 TLS client fingerprint from a ClientHello",
    skill(
        description = "Compute the JA3 TLS client fingerprint from a raw ClientHello given as a hex string. JA3 fingerprints a TLS client by concatenating five decimal fields from its ClientHello in the order SSLVersion,Ciphers,Extensions,EllipticCurves,EllipticCurvePointFormats (cipher suites, extension types, supported_groups, and ec_point_formats joined by '-'), with GREASE values removed (RFC 8701). The MD5 of that string is the JA3 hash seen in IDS / proxy / threat-intel logs (e.g. Suricata, Zeek). Also returns JA3N, the normalized variant that sorts the extension list ascending so the fingerprint is stable across the TLS extension-order randomization used by modern browsers (Chrome 110+, Firefox 114+). Input may begin at the TLS record header (16 03 ...), the handshake header (01 ...), or the ClientHello body, and may contain spaces, colons, dashes, dots, commas, or a 0x prefix. Returns the JA3 and JA3N strings and their MD5 hashes, the legacy TLS version, the decimal cipher suites / extensions / elliptic curves / point formats, and any SNI server names. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Ja3Fingerprint {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ja3-fingerprint", |a: Args| {
            compute_ja3(&a.client_hello).map_err(SkillError::InvalidArgs)
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
                    "client_hello": { "type": "string", "description": "The TLS ClientHello as a hex string. It may start at the 5-byte TLS record header (begins '16 03 ...'), the handshake header (begins '01 ...'), or directly at the ClientHello body. Spaces, colons, dashes, dots, commas, and a 0x prefix are ignored." }
                },
                "required": ["client_hello"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
