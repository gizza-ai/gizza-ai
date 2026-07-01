//! gizza-ai/asn1-parser — parse an ASN.1 / DER hex string into a readable tree
//! of tags, lengths, OIDs and decoded primitive values. Chat schema is
//! single-sourced from descriptor(); handle() delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_asn1_parser_core::parse;
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "tree".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The ASN.1/DER structure as a hex string. Spaces, colons, newlines and a leading 0x are ignored."),
        )
        .param(
            Param::enumv("format", ["tree", "json"]).default("tree").describe(
                "tree (default) renders an indented text tree; json returns the parse tree as structured JSON.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_logic(a: Args) -> Result<String, String> {
    parse(&a.input, &a.format)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/asn1-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an ASN.1/DER hex string into a readable tree",
    skill(
        description = "Parse an ASN.1 / DER (Distinguished Encoding Rules) byte stream — supplied as a hex string — into a readable tree of tag-length-value elements. Decodes universal tags (SEQUENCE, SET, INTEGER, BOOLEAN, OCTET STRING, BIT STRING, NULL, OBJECT IDENTIFIER, UTF8String/PrintableString/IA5String and the other string types, UTCTime, GeneralizedTime, ENUMERATED), context-specific/application/private tags, and both short- and long-form lengths and high-tag-number tags. OBJECT IDENTIFIERs are shown in dotted form with a friendly name for common PKI/X.509 OIDs (e.g. 1.2.840.113549.1.1.11 → sha256WithRSAEncryption, 2.5.4.3 → commonName). Integers, booleans, times and strings are decoded to human-readable values; BIT STRING / OCTET STRING contents are best-effort recursed when they wrap a nested DER structure (as certificates nest a public key or extension value). format=tree (default) gives an indented text tree; format=json returns the parse tree as structured JSON. Useful for inspecting X.509 certificates, CSRs, private/public keys, and other DER/BER data. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "asn1-parser", |a: Args| {
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
    fn tree_path() {
        let r = run_logic(Args {
            input: "02012A".to_string(),
            format: "tree".to_string(),
        })
        .unwrap();
        assert_eq!(r, "INTEGER (0x02) len=1 @0: 42 (0x2A)");
    }

    #[test]
    fn json_path() {
        let r = run_logic(Args {
            input: "0500".to_string(),
            format: "json".to_string(),
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["tag"], "NULL");
    }

    #[test]
    fn bad_input_errors() {
        let r = run_logic(Args {
            input: "0203FFFF".to_string(),
            format: "tree".to_string(),
        });
        assert!(r.is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input":  { "type": "string", "description": "The ASN.1/DER structure as a hex string. Spaces, colons, newlines and a leading 0x are ignored." },
                    "format": { "type": "string", "enum": ["tree", "json"], "default": "tree", "description": "tree (default) renders an indented text tree; json returns the parse tree as structured JSON." }
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
