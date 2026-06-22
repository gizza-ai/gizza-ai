//! gizza-ai/aes-key-wrap — wrap or unwrap cryptographic key material with AES Key Wrap
//! (KW = RFC 3394, KWP = RFC 5649), 128/192/256-bit KEKs, hex/base64 I/O. Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_aes_key_wrap_core::{unwrap, wrap, Algorithm, Encoding};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_op")]
    operation: String,
    key: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_format")]
    format: String,
}
fn default_op() -> String {
    "wrap".to_string()
}
fn default_algorithm() -> String {
    "kw".to_string()
}
fn default_format() -> String {
    "base64".to_string()
}

#[derive(Serialize)]
struct Resp {
    output: String,
    operation: String,
    algorithm: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For wrap: the key material to protect, encoded with `format`. For unwrap: the wrapped blob.",
        ))
        .param(
            Param::enumv("operation", ["wrap", "unwrap"]).default("wrap").describe(
                "wrap (default) protects a key under the KEK; unwrap recovers it and verifies the integrity check.",
            ),
        )
        .param(Param::string("key").required().describe(
            "The key-encryption key (KEK), encoded with `format`. Its length selects the cipher: 16 bytes = AES-128, 24 = AES-192, 32 = AES-256.",
        ))
        .param(
            Param::enumv("algorithm", ["kw", "kwp"]).default("kw").describe(
                "kw (default, RFC 3394 / NIST SP 800-38F) wraps key material a multiple of 8 bytes and ≥ 16 bytes; kwp (RFC 5649, \"with padding\") wraps any length ≥ 1 byte.",
            ),
        )
        .param(
            Param::enumv("format", ["base64", "hex"]).default("base64").describe(
                "Encoding for the KEK, the key material, and the wrapped output (base64 default, or hex).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AesKeyWrap;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/aes-key-wrap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Wrap or unwrap cryptographic keys with AES Key Wrap (RFC 3394 / RFC 5649)",
    skill(
        description = "Wrap or unwrap cryptographic key material with the AES Key Wrap algorithm. operation = wrap (default, protect a key under a key-encryption key) or unwrap (recover it, verifying the integrity check). algorithm = kw (default, RFC 3394 / NIST SP 800-38F; key material must be a multiple of 8 bytes and at least 16 bytes) or kwp (RFC 5649, \"with padding\"; any length >= 1 byte). The key (KEK) length selects AES-128/192/256 (16/24/32 bytes). The data, key and wrapped output are hex or base64 (set format; default base64). Wrapping adds 8 bytes of integrity check; unwrapping fails if the KEK, algorithm, or data is wrong. To encrypt arbitrary plaintext rather than a key, use aes-cipher or text-encrypt instead.",
        parameters = schema_json()
    ),
)]
impl AesKeyWrap {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "aes-key-wrap", |a: Args| {
            let alg = Algorithm::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "wrap" | "" => wrap(&a.data, &a.key, alg, fmt),
                "unwrap" => unwrap(&a.data, &a.key, alg, fmt),
                other => Err(format!("unknown operation '{other}' (use wrap or unwrap)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output, operation: a.operation, algorithm: a.algorithm })
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
                    "data": { "type": "string", "description": "For wrap: the key material to protect, encoded with `format`. For unwrap: the wrapped blob." },
                    "operation": { "type": "string", "enum": ["wrap", "unwrap"], "default": "wrap", "description": "wrap (default) protects a key under the KEK; unwrap recovers it and verifies the integrity check." },
                    "key": { "type": "string", "description": "The key-encryption key (KEK), encoded with `format`. Its length selects the cipher: 16 bytes = AES-128, 24 = AES-192, 32 = AES-256." },
                    "algorithm": { "type": "string", "enum": ["kw", "kwp"], "default": "kw", "description": "kw (default, RFC 3394 / NIST SP 800-38F) wraps key material a multiple of 8 bytes and ≥ 16 bytes; kwp (RFC 5649, \"with padding\") wraps any length ≥ 1 byte." },
                    "format": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "Encoding for the KEK, the key material, and the wrapped output (base64 default, or hex)." }
                },
                "required": ["data", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
