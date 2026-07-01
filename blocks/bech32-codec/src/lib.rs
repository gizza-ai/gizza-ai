//! gizza-ai/bech32-codec — encode an HRP + data to a checksummed Bech32 (BIP 173)
//! or Bech32m (BIP 350) string, and decode/validate Bech32 strings. Chat schema is
//! single-sourced from descriptor() (shared with CLI); the handler delegates to
//! block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    hrp: String,
    #[serde(default)]
    variant: String,
    #[serde(default)]
    format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("On encode: the data payload (hex bytes or UTF-8 text per 'format'). On decode: the Bech32/Bech32m string to validate and unpack."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) builds a checksummed Bech32 string from an HRP + data; 'decode' validates a Bech32 string and returns its HRP, variant, and data."),
        )
        .param(
            Param::string("hrp")
                .describe("Encode only: the human-readable prefix, e.g. 'bc' (Bitcoin mainnet), 'tb' (testnet), 'lnbc' (Lightning), 'npub' (Nostr). Lower-cased in output. Ignored on decode (read from the string)."),
        )
        .param(
            Param::enumv("variant", ["bech32", "bech32m"])
                .default("bech32")
                .describe("Checksum variant to write on encode: 'bech32' (BIP 173, SegWit v0) or 'bech32m' (BIP 350, Taproot/v1+). On decode the variant is auto-detected and reported."),
        )
        .param(
            Param::enumv("format", ["hex", "text"])
                .default("hex")
                .describe("How to read the payload on encode, or render it on decode. 'hex' (default) is a hex byte string for binary data; 'text' is UTF-8."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_logic(a: Args) -> Result<String, String> {
    gizza_ai_bech32_codec_core::convert(&a.input, &a.mode, &a.hrp, &a.variant, &a.format)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bech32-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode and decode Bech32 / Bech32m strings",
    skill(
        description = "Encode a human-readable prefix (HRP) plus data into a checksummed Bech32 (BIP 173) or Bech32m (BIP 350) string, or decode/validate a Bech32 string back to its HRP, variant, and data. Bech32 is the encoding behind SegWit Bitcoin addresses, Lightning invoices, Nostr npub/nsec keys, and Cosmos addresses. Use mode='encode' (default) with an 'hrp' (e.g. bc, tb, lnbc) and a data payload, choosing variant='bech32' or 'bech32m'; the tool computes the base-32 data part and the 6-character BCH checksum. Use mode='decode' to validate a string and get back its prefix, detected variant, and data (variant is auto-detected). Use format='hex' (default) for binary payloads or format='text' for UTF-8. Data bytes are converted to/from 5-bit groups per the BIP 173 convertbits algorithm. Runs locally and never uploads data.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bech32-codec", |a: Args| {
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
    fn encode_path() {
        let r = run_logic(Args {
            input: "751e76e8199196d454941c45d1b3a323f1433bd6".to_string(),
            mode: "encode".to_string(),
            hrp: "bc".to_string(),
            variant: "bech32".to_string(),
            format: "hex".to_string(),
        })
        .unwrap();
        assert!(r.starts_with("bc1"), "got {r}");
    }

    #[test]
    fn decode_path_reports_variant() {
        let r = run_logic(Args {
            input: "A1LQFN3A".to_string(),
            mode: "decode".to_string(),
            hrp: String::new(),
            variant: String::new(),
            format: "hex".to_string(),
        })
        .unwrap();
        assert!(r.contains("variant: bech32m"), "got {r}");
        assert!(r.contains("hrp: a"), "got {r}");
    }

    #[test]
    fn invalid_variant_errors() {
        let r = run_logic(Args {
            input: "00".to_string(),
            mode: "encode".to_string(),
            hrp: "bc".to_string(),
            variant: "base99".to_string(),
            format: "hex".to_string(),
        });
        assert!(r.is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "On encode: the data payload (hex bytes or UTF-8 text per 'format'). On decode: the Bech32/Bech32m string to validate and unpack." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) builds a checksummed Bech32 string from an HRP + data; 'decode' validates a Bech32 string and returns its HRP, variant, and data." },
                    "hrp": { "type": "string", "description": "Encode only: the human-readable prefix, e.g. 'bc' (Bitcoin mainnet), 'tb' (testnet), 'lnbc' (Lightning), 'npub' (Nostr). Lower-cased in output. Ignored on decode (read from the string)." },
                    "variant": { "type": "string", "enum": ["bech32", "bech32m"], "default": "bech32", "description": "Checksum variant to write on encode: 'bech32' (BIP 173, SegWit v0) or 'bech32m' (BIP 350, Taproot/v1+). On decode the variant is auto-detected and reported." },
                    "format": { "type": "string", "enum": ["hex", "text"], "default": "hex", "description": "How to read the payload on encode, or render it on decode. 'hex' (default) is a hex byte string for binary data; 'text' is UTF-8." }
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
