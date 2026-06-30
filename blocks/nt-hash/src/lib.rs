//! gizza-ai/nt-hash — chat skill block on the shared tool abstraction.
//!
//! Computes the NT (NTLM) hash of a password: `MD4(UTF-16LE(password))`, a
//! 128-bit digest rendered as 32 hex chars (default) or base64. Pure Rust
//! (RustCrypto `md4`) → runs on ALL backends including the chat Service Worker.
//! Surfaces: chat + CLI + standalone page.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    password: String,
    #[serde(default)]
    output_format: String,
    #[serde(default)]
    uppercase: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("password")
                .required()
                .describe("The password (or any text) to hash. It is UTF-16LE encoded, then MD4-hashed, to produce the NT/NTLM hash."),
        )
        .param(
            Param::enumv("output_format", ["hex", "base64"])
                .default("hex")
                .describe("Digest representation. 'hex' (default) is 32 lowercase hex chars — the conventional NTLM form; 'base64' is standard base64 (24 chars)."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("When output_format is hex, emit uppercase hex. No effect on base64. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nt-hash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the NT (NTLM) hash of a password",
    skill(
        description = "Compute the NT hash (also called the NTLM hash or NTOWF) of a password. The NT hash is MD4(UTF-16LE(password)): the password is encoded as little-endian UTF-16, then MD4-hashed, giving a 128-bit digest returned by default as 32 lowercase hex chars — the value stored in the Windows SAM/NTDS.dit and used by NTLM authentication and pass-the-hash. Set output_format='base64' for a base64 digest, or uppercase=true for uppercase hex. NOTE: the NT hash is UNSALTED and MD4 is cryptographically broken, so it provides essentially no protection against offline cracking — use it for password audits, CTFs, and NTLM/pass-the-hash interop, NOT for storing new passwords (use argon2-hash or bcrypt-hash for that). To hash text with a modern algorithm, use sha256-hash.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nt-hash", |a: Args| {
            gizza_ai_nt_hash_core::hash(&a.password, &a.output_format, a.uppercase)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "password": { "type": "string", "description": "The password (or any text) to hash. It is UTF-16LE encoded, then MD4-hashed, to produce the NT/NTLM hash." },
                    "output_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Digest representation. 'hex' (default) is 32 lowercase hex chars — the conventional NTLM form; 'base64' is standard base64 (24 chars)." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When output_format is hex, emit uppercase hex. No effect on base64. Default false." }
                },
                "required": ["password"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
