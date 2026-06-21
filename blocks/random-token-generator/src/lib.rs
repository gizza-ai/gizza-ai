//! gizza-ai/random-token-generator — generate cryptographically random tokens
//! with a configurable length and character set. Thin wrapper; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure
//! (getrandom CSPRNG) → all backends incl. the chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_random_token_generator_core::generate;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_length")]
    length: u32,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default = "default_charset")]
    charset: String,
    #[serde(default)]
    custom_chars: String,
}
fn default_length() -> u32 { 32 }
fn default_count() -> u32 { 1 }
fn default_charset() -> String { "hex".to_string() }

#[derive(Serialize)]
struct Resp {
    tokens: Vec<String>,
    bits: f64,
    charset_size: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::integer("length").min(1.0).max(4096.0).describe("Token length in characters (default 32)."))
        .param(Param::integer("count").min(1.0).max(1000.0).describe("How many tokens to generate (default 1)."))
        .param(Param::enumv("charset", ["hex", "hex-upper", "base64url", "alphanumeric", "alphabetic", "numeric", "safe"]).default("hex").describe("Character set: 'hex' (default), 'hex-upper', 'base64url', 'alphanumeric' (base62), 'alphabetic', 'numeric', or 'safe' (alphanumeric minus look-alike 0/O/1/l/I). Ignored when custom_chars is set."))
        .param(Param::string("custom_chars").default("").describe("Optional custom alphabet: when non-empty, tokens are drawn from these characters (duplicates removed, min 2 distinct) instead of the charset preset."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RandomTokenGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/random-token-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate cryptographically random tokens",
    skill(
        description = "Generate one or more cryptographically random tokens locally with a CSPRNG. Each token is `length` characters drawn uniformly (no modulo bias) from a `charset` preset — 'hex' (default), 'hex-upper', 'base64url', 'alphanumeric' (base62), 'alphabetic', 'numeric', or 'safe' (alphanumeric without look-alike characters) — or from `custom_chars` when provided. `count` returns a batch. Returns the tokens, their per-token entropy in bits, and the alphabet size.",
        parameters = schema_json()
    )
)]
impl RandomTokenGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "random-token-generator", |a: Args| {
            let t = generate(
                a.length as usize,
                a.count as usize,
                &a.charset,
                &a.custom_chars,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                tokens: t.tokens,
                bits: t.bits,
                charset_size: t.charset_size,
            })
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
                    "length":  { "type": "integer", "minimum": 1, "maximum": 4096, "description": "Token length in characters (default 32)." },
                    "count":   { "type": "integer", "minimum": 1, "maximum": 1000, "description": "How many tokens to generate (default 1)." },
                    "charset": { "type": "string", "enum": ["hex", "hex-upper", "base64url", "alphanumeric", "alphabetic", "numeric", "safe"], "default": "hex", "description": "Character set: 'hex' (default), 'hex-upper', 'base64url', 'alphanumeric' (base62), 'alphabetic', 'numeric', or 'safe' (alphanumeric minus look-alike 0/O/1/l/I). Ignored when custom_chars is set." },
                    "custom_chars": { "type": "string", "default": "", "description": "Optional custom alphabet: when non-empty, tokens are drawn from these characters (duplicates removed, min 2 distinct) instead of the charset preset." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
