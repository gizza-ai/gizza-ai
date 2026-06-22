//! gizza-ai/bencode-decoder — convert between bencode (the BitTorrent
//! serialization format) and JSON, in both directions. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_bencode_decoder_core::{decode_to_json, encode_from_json};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_mode() -> String {
    "decode".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The data to convert: bencode text in decode mode, or JSON text in encode mode."),
        )
        .param(
            Param::enumv("mode", ["decode", "encode"]).default("decode").describe(
                "decode (default) parses bencode into JSON; encode serializes JSON into bencode.",
            ),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the JSON output in decode mode. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Shared logic so the wasm handler and CLI both go through one path.
/// Returns the converted text: JSON (decode) or bencode (encode).
fn run_logic(a: Args) -> Result<String, String> {
    match a.mode.trim().to_ascii_lowercase().as_str() {
        "decode" | "" => decode_to_json(a.input.as_bytes(), a.pretty),
        "encode" => {
            let bytes = encode_from_json(&a.input)?;
            // Bencode is mostly ASCII; surface it as a UTF-8 string. Binary byte
            // strings (only produced from a `_bencode_bytes_hex` sentinel) are
            // rendered lossily here — for exact bytes, keep the data as JSON.
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        }
        other => Err(format!("unknown mode '{other}' (use decode or encode)")),
    }
}

#[cfg(target_arch = "wasm32")]
struct BencodeDecoder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bencode-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between bencode (torrent format) and JSON",
    skill(
        description = "Convert between bencode — the BitTorrent serialization format used by .torrent files and the tracker/DHT protocols — and JSON, in both directions. mode=decode (default) parses bencode text (integers i…e, byte strings <len>:…, lists l…e, dictionaries d…e) into readable JSON; mode=encode serializes JSON back into canonical bencode (dictionary keys are sorted by byte value). pretty indents the decode-mode JSON (default true). Non-UTF-8 byte strings (e.g. a torrent's binary `pieces` field) are carried losslessly as {\"_bencode_bytes_hex\":\"…\"} so decode then encode reproduces the exact bytes. Note: bencode has no boolean or null type. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl BencodeDecoder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bencode-decoder", |a: Args| {
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
    fn decode_path() {
        let r = run_logic(Args {
            input: "d3:cow3:mooe".to_string(),
            mode: "decode".to_string(),
            pretty: false,
        })
        .unwrap();
        assert_eq!(r, "{\"cow\":\"moo\"}");
    }

    #[test]
    fn encode_path() {
        let r = run_logic(Args {
            input: "{\"cow\":\"moo\"}".to_string(),
            mode: "encode".to_string(),
            pretty: true,
        })
        .unwrap();
        assert_eq!(r, "d3:cow3:mooe");
    }

    #[test]
    fn unknown_mode_errors() {
        let r = run_logic(Args {
            input: "x".to_string(),
            mode: "frobnicate".to_string(),
            pretty: true,
        });
        assert!(r.is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input":  { "type": "string", "description": "The data to convert: bencode text in decode mode, or JSON text in encode mode." },
                    "mode":   { "type": "string", "enum": ["decode", "encode"], "default": "decode", "description": "decode (default) parses bencode into JSON; encode serializes JSON into bencode." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent the JSON output in decode mode. Default true." }
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
