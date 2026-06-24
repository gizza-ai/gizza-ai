//! gizza-ai/ja4-server-fingerprint — compute the JA4S TLS server fingerprint
//! from a raw ServerHello given as a hex string. Chat schema is single-sourced
//! from descriptor(); handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ja4_server_fingerprint_core::run_with as compute_ja4s;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    server_hello: String,
    #[serde(default)]
    quic: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("server_hello")
                .required()
                .describe("The TLS ServerHello as a hex string. It may start at the 5-byte TLS record header (begins '16 03 ...'), the handshake header (begins '02 ...'), or directly at the ServerHello body. Spaces, colons, dashes, dots, commas, and a 0x prefix are ignored."),
        )
        .param(
            Param::boolean("quic")
                .default(false)
                .describe("Set true if this ServerHello came over QUIC (uses the 'q' transport prefix); leave false for TCP ('t')."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Ja4ServerFingerprint;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ja4-server-fingerprint",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the JA4S TLS server fingerprint from a ServerHello",
    skill(
        description = "Compute the JA4S TLS server fingerprint from a raw ServerHello given as a hex string. JA4S (FoxIO, part of the JA4+ suite) fingerprints a TLS server's response. The fingerprint has the form a_b_c: a = transport char (t = TCP, q = QUIC) + TLS version (13,12,11,10,s3,s2 — from the supported_versions extension if present, else legacy_version) + the 2-digit extension count + the ALPN field (first+last char of the chosen ALPN protocol, 00 if none); b = the single chosen cipher suite as 4 hex chars; c = the first 12 hex chars of the SHA256 of the comma-joined extension type list (4-char hex, wire order, GREASE kept; 000000000000 if there are no extensions). Input may begin at the TLS record header (16 03 ...), the handshake header (02 ...), or the ServerHello body, and may contain spaces, colons, dashes, dots, commas, or a 0x prefix. Set quic=true for a QUIC handshake. Returns the JA4S string, the raw JA4S_r variant (extension list un-hashed), the negotiated TLS version, the chosen cipher, the extension list, and the selected ALPN protocol. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Ja4ServerFingerprint {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ja4-server-fingerprint", |a: Args| {
            compute_ja4s(&a.server_hello, a.quic).map_err(SkillError::InvalidArgs)
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
                    "server_hello": { "type": "string", "description": "The TLS ServerHello as a hex string. It may start at the 5-byte TLS record header (begins '16 03 ...'), the handshake header (begins '02 ...'), or directly at the ServerHello body. Spaces, colons, dashes, dots, commas, and a 0x prefix are ignored." },
                    "quic": { "type": "boolean", "default": false, "description": "Set true if this ServerHello came over QUIC (uses the 'q' transport prefix); leave false for TCP ('t')." }
                },
                "required": ["server_hello"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
