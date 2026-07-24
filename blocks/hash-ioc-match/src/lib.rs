//! gizza-ai/hash-ioc-match — chat skill block on the shared tool abstraction.
//!
//! Hashes an input (a file's bytes, given as text/hex/base64) with the four hash
//! families threat-intel IOC feeds use — MD5, SHA-1, SHA-256, SHA-512 — and
//! flags it if any digest appears in a pasted blocklist of known-bad hashes.
//! Pure Rust (RustCrypto) → runs on ALL backends including the chat Service
//! Worker. Surfaces: chat + CLI + page.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    blocklist: String,
    #[serde(default)]
    input_encoding: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The file content to hash and check, as text, hex, or base64 (set `input_encoding` to match). The tool computes its MD5, SHA-1, SHA-256 and SHA-512 digests."),
        )
        .param(
            Param::string("blocklist")
                .required()
                .describe("Pasted list of known-bad hashes to match against — one per line or any format. Labelled ('MD5: <hash>'), CSV ('<hash>,name.exe'), '0x'-prefixed and '#'-commented lines are all parsed; only 32/40/64/128-char hex runs (MD5/SHA-1/SHA-256/SHA-512 widths) are kept, case-insensitively."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `input` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hash-ioc-match",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hash a file and flag it against a blocklist of known-bad IOC hashes.",
    skill(
        description = "Hash a piece of content and check it against a blocklist of known-bad file hashes. Provide the `input` (a file's bytes as text, hex, or base64 via `input_encoding`) and a `blocklist` of known-bad hashes pasted in any format. The tool computes the input's MD5, SHA-1, SHA-256 and SHA-512 digests and reports FLAGGED if any of them appears in the blocklist, or CLEAN otherwise, listing every computed digest and which one matched. The blocklist is parsed leniently: labelled ('MD5: <hash>'), CSV ('<hash>,malware.exe'), '0x'-prefixed and '#'/';'-commented lines all work — any 32/40/64/128-char hex run (MD5/SHA-1/SHA-256/SHA-512 widths) is extracted, case-insensitively, and duplicates collapse. Use this to triage a sample against threat-intel IOC feeds. To just compute a hash use the hash-text tool; to pull hashes out of a report use the ioc-extract tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hash-ioc-match", |a: Args| {
            gizza_ai_hash_ioc_match_core::report(&a.input, &a.blocklist, &a.input_encoding)
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
                    "input": { "type": "string", "description": "The file content to hash and check, as text, hex, or base64 (set `input_encoding` to match). The tool computes its MD5, SHA-1, SHA-256 and SHA-512 digests." },
                    "blocklist": { "type": "string", "description": "Pasted list of known-bad hashes to match against — one per line or any format. Labelled ('MD5: <hash>'), CSV ('<hash>,name.exe'), '0x'-prefixed and '#'-commented lines are all parsed; only 32/40/64/128-char hex runs (MD5/SHA-1/SHA-256/SHA-512 widths) are kept, case-insensitively." },
                    "input_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `input` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first." }
                },
                "required": ["input", "blocklist"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
