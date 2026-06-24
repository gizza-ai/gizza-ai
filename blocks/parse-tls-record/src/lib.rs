//! gizza-ai/parse-tls-record — decode TLS record-layer bytes (given as hex) into
//! the record header (content type, version, length) and, for handshake records,
//! the handshake messages — fully decoding ClientHello / ServerHello (version,
//! random, session id, cipher suites by name, compression, and extensions: SNI,
//! ALPN, supported_versions, supported_groups, signature_algorithms, key_share).
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_parse_tls_record_core::run as parse_record;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    record: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("record")
            .required()
            .describe("The TLS record-layer bytes as a hex string, starting at the 5-byte record header (content type, version, length). Spaces, colons, dashes, dots, commas, and a 0x prefix are ignored, e.g. a handshake record beginning '160301...'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseTlsRecord;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-tls-record",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode TLS record-layer bytes from hex",
    skill(
        description = "Decode TLS record-layer bytes given as a hex string into structured fields. Handles one or more concatenated records (e.g. ServerHello followed by a Certificate record): for each, the 5-byte record header (content type — change_cipher_spec/alert/handshake/application_data/heartbeat, the record protocol version, and the declared length) and, for handshake records, the handshake messages. ClientHello and ServerHello are fully decoded: legacy/record version, the 32-byte random, the session id, the offered or selected cipher suites named by their IANA identifier (e.g. TLS_AES_128_GCM_SHA256, TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256), the compression methods, and the extensions — including SNI server names, ALPN protocols, supported_versions, supported_groups, signature_algorithms, and key_share groups. Alert records are decoded to level + description. Input may contain spaces, colons, dashes, dots, commas, or a 0x prefix. Returns JSON (a single object for one record, an array for several). Runs locally.",
        parameters = schema_json()
    ),
)]
impl ParseTlsRecord {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-tls-record", |a: Args| {
            parse_record(&a.record).map_err(SkillError::InvalidArgs)
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
                    "record": { "type": "string", "description": "The TLS record-layer bytes as a hex string, starting at the 5-byte record header (content type, version, length). Spaces, colons, dashes, dots, commas, and a 0x prefix are ignored, e.g. a handshake record beginning '160301...'." }
                },
                "required": ["record"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
