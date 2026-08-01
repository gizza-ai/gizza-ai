//! gizza-ai/nostr-key-encoder — convert Nostr identifiers between raw hex and
//! their NIP-19 bech32 forms (npub/nsec/note/nprofile/nevent). The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    mode: String,
    #[serde(default = "default_type")]
    r#type: String,
    #[serde(default)]
    relays: String,
    #[serde(default)]
    author: String,
    #[serde(default = "default_kind")]
    kind: f64,
}

fn default_type() -> String {
    "npub".to_string()
}
fn default_kind() -> f64 {
    -1.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The identifier to convert: either a raw 64-char hex key/id (to encode) or a NIP-19 bech32 string like npub1…/nsec1…/note1…/nprofile1…/nevent1… (to decode). A leading 0x and whitespace are ignored on hex input."),
        )
        .param(
            Param::enumv("mode", ["auto", "encode", "decode"])
                .default("auto")
                .describe("Direction. 'auto' (default) decodes if the input is a NIP-19 bech32 string and encodes otherwise; 'encode' forces hex→bech32 using 'type'; 'decode' forces bech32→hex/report."),
        )
        .param(
            Param::enumv("type", ["npub", "nsec", "note", "nprofile", "nevent"])
                .default("npub")
                .describe("Target NIP-19 prefix when ENCODING hex. 'npub' public key, 'nsec' private key, 'note' event id (all 32-byte); 'nprofile' pubkey + relays; 'nevent' event id + optional relays/author/kind. Ignored on decode (the prefix is read from the string)."),
        )
        .param(
            Param::string("relays")
                .describe("Encode only (nprofile/nevent): optional relay URLs where the entity can be found, e.g. 'wss://relay.example.com'. Separate multiple with commas, spaces, or newlines. Ignored for bare types and on decode."),
        )
        .param(
            Param::string("author")
                .describe("Encode only (nevent): optional 64-char hex public key of the event author, embedded as a TLV hint. Ignored for other types and on decode."),
        )
        .param(
            Param::integer("kind")
                .default(-1.0)
                .describe("Encode only (nevent): optional Nostr event kind as a 32-bit unsigned integer (0..=4294967295), e.g. 1 for a text note. Use -1 (default) to omit it. Ignored for other types and on decode."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_logic(a: Args) -> Result<String, String> {
    gizza_ai_nostr_key_encoder_core::convert(
        &a.input,
        &a.mode,
        &a.r#type,
        &a.relays,
        &a.author,
        a.kind as i64,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nostr-key-encoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Nostr identifiers between hex and NIP-19 bech32",
    skill(
        description = "Convert Nostr identifiers between raw hex and their NIP-19 bech32 forms. Encode a 64-char hex key/id into npub (public key), nsec (private key), or note (event id), or into the TLV forms nprofile (pubkey + relays) and nevent (event id + optional relays, author, kind). Decode any NIP-19 string (npub/nsec/note/nprofile/nevent, plus naddr/nrelay) back to hex or a labeled report. Set mode='auto' (default) to detect direction, or force 'encode'/'decode'. Nostr uses the plain Bech32 (BIP 173) checksum with no 90-char length cap. Runs locally; never uploads keys.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nostr-key-encoder", |a: Args| {
            run_logic(a).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The identifier to convert: either a raw 64-char hex key/id (to encode) or a NIP-19 bech32 string like npub1…/nsec1…/note1…/nprofile1…/nevent1… (to decode). A leading 0x and whitespace are ignored on hex input." },
                    "mode": { "type": "string", "enum": ["auto", "encode", "decode"], "default": "auto", "description": "Direction. 'auto' (default) decodes if the input is a NIP-19 bech32 string and encodes otherwise; 'encode' forces hex→bech32 using 'type'; 'decode' forces bech32→hex/report." },
                    "type": { "type": "string", "enum": ["npub", "nsec", "note", "nprofile", "nevent"], "default": "npub", "description": "Target NIP-19 prefix when ENCODING hex. 'npub' public key, 'nsec' private key, 'note' event id (all 32-byte); 'nprofile' pubkey + relays; 'nevent' event id + optional relays/author/kind. Ignored on decode (the prefix is read from the string)." },
                    "relays": { "type": "string", "description": "Encode only (nprofile/nevent): optional relay URLs where the entity can be found, e.g. 'wss://relay.example.com'. Separate multiple with commas, spaces, or newlines. Ignored for bare types and on decode." },
                    "author": { "type": "string", "description": "Encode only (nevent): optional 64-char hex public key of the event author, embedded as a TLV hint. Ignored for other types and on decode." },
                    "kind": { "type": "integer", "default": -1.0, "description": "Encode only (nevent): optional Nostr event kind as a 32-bit unsigned integer (0..=4294967295), e.g. 1 for a text note. Use -1 (default) to omit it. Ignored for other types and on decode." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
