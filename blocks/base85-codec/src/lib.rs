//! gizza-ai/base85-codec — encode and decode Ascii85, Z85, and RFC 1924
//! Base85. Chat schema is single-sourced from descriptor() (shared with CLI)
//! and the handler delegates to block_utils::run_skill.
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
    variant: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    adobe_frame: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The text or hex bytes to encode, or the Base85 string to decode."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns data into Base85, 'decode' reverses it."),
        )
        .param(
            Param::enumv("variant", ["ascii85", "z85", "rfc1924"])
                .default("ascii85")
                .describe("Base85 flavour. 'ascii85' is Adobe/btoa with the z zero shortcut and optional <~ ~> framing; 'z85' is ZeroMQ Z85 and requires byte lengths divisible by 4; 'rfc1924' uses the RFC 1924 alphabet."),
        )
        .param(
            Param::enumv("format", ["text", "hex"])
                .default("text")
                .describe("How to read input when encoding, or render decoded output. 'text' (default) is UTF-8; 'hex' is a hex byte string for binary data."),
        )
        .param(
            Param::boolean("adobe_frame")
                .default(false)
                .describe("Ascii85 encode only: wrap output in Adobe <~ and ~> delimiters. Ignored for Z85/RFC 1924; decode strips Ascii85 framing automatically."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_logic(a: Args) -> Result<String, String> {
    gizza_ai_base85_codec_core::convert(&a.input, &a.mode, &a.variant, &a.format, a.adobe_frame)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base85-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode and decode Ascii85, Z85, and RFC 1924 Base85",
    skill(
        description = "Encode text or hex bytes to Base85, or decode Base85 back to text or hex. Supports three variants: ascii85 (Adobe/btoa alphabet with the z all-zero shortcut, partial final groups, optional <~ ~> framing on encode, and automatic framing removal on decode), z85 (ZeroMQ RFC 32 alphabet; input bytes must be a multiple of 4 and encoded strings a multiple of 5), and rfc1924 (the RFC 1924 / Python b85 alphabet with partial final groups). Use mode='encode' (default) or mode='decode'. Use format='text' (default) for UTF-8, or format='hex' for binary data. Set adobe_frame=true to wrap Ascii85 encoded output in <~ and ~>. Runs locally and never uploads data.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "base85-codec", |a: Args| {
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
    fn ascii85_encode_path() {
        let r = run_logic(Args {
            input: "Hello World!".to_string(),
            mode: "encode".to_string(),
            variant: "ascii85".to_string(),
            format: "text".to_string(),
            adobe_frame: false,
        })
        .unwrap();
        assert_eq!(r, "87cURD]i,\"Ebo80");
    }

    #[test]
    fn z85_hex_decode_path() {
        let r = run_logic(Args {
            input: "HelloWorld".to_string(),
            mode: "decode".to_string(),
            variant: "z85".to_string(),
            format: "hex".to_string(),
            adobe_frame: false,
        })
        .unwrap();
        assert_eq!(r, "864fd26fb559f75b");
    }

    #[test]
    fn invalid_variant_errors() {
        let r = run_logic(Args {
            input: "x".to_string(),
            mode: "encode".to_string(),
            variant: "base999".to_string(),
            format: "text".to_string(),
            adobe_frame: false,
        });
        assert!(r.is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The text or hex bytes to encode, or the Base85 string to decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns data into Base85, 'decode' reverses it." },
                    "variant": { "type": "string", "enum": ["ascii85", "z85", "rfc1924"], "default": "ascii85", "description": "Base85 flavour. 'ascii85' is Adobe/btoa with the z zero shortcut and optional <~ ~> framing; 'z85' is ZeroMQ Z85 and requires byte lengths divisible by 4; 'rfc1924' uses the RFC 1924 alphabet." },
                    "format": { "type": "string", "enum": ["text", "hex"], "default": "text", "description": "How to read input when encoding, or render decoded output. 'text' (default) is UTF-8; 'hex' is a hex byte string for binary data." },
                    "adobe_frame": { "type": "boolean", "default": false, "description": "Ascii85 encode only: wrap output in Adobe <~ and ~> delimiters. Ignored for Z85/RFC 1924; decode strips Ascii85 framing automatically." }
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
