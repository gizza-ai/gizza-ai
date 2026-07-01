//! gizza-ai/hash-identifier — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args { input: String }

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The hash or digest string to identify (e.g. a bcrypt '$2b$...' string, an Argon2 PHC string, or a bare hex digest like an MD5/SHA-256)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hash-identifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Identify the likely hash/MAC scheme of a digest string.",
    skill(
        description = "Identify the likely hash or MAC scheme that produced a given digest string. Recognition is structural: prefixed formats (bcrypt '$2b$...', Argon2 '$argon2id$...', sha512crypt '$6$...', PHPass/WordPress '$P$...', Apache apr1, Cisco type 8/9, LDAP {SSHA}, MySQL '*...', NetNTLM) are matched by their unambiguous prefix and reported with high confidence; a bare fixed-width hex digest can match a whole family of same-width algorithms (e.g. a 32-hex string is MD5 OR NTLM OR MD4) so all plausible candidates are listed, best-confidence first. Pass the single hash string in 'input'. This does NOT crack hashes or recover the original value; it only classifies the format.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "hash-identifier", |a: Args| {
            gizza_ai_hash_identifier_core::run(&a.input).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The hash or digest string to identify (e.g. a bcrypt '$2b$...' string, an Argon2 PHC string, or a bare hex digest like an MD5/SHA-256)." }
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
