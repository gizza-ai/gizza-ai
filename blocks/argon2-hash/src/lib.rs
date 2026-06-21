//! gizza-ai/argon2-hash — hash a password with Argon2id (or verify one against a
//! PHC string). Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_argon2_hash_core::{hash, verify};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    password: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    hash: String,
    #[serde(default = "default_memory")]
    memory_kib: u32,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(default = "default_parallelism")]
    parallelism: u32,
}
fn default_mode() -> String {
    "hash".to_string()
}
fn default_memory() -> u32 {
    19456
}
fn default_iterations() -> u32 {
    2
}
fn default_parallelism() -> u32 {
    1
}

#[derive(Serialize)]
struct Resp {
    #[serde(skip_serializing_if = "Option::is_none")]
    phc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "match")]
    matches: Option<bool>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("password").required().describe("The password to hash, or to verify."))
        .param(
            Param::enumv("mode", ["hash", "verify"]).default("hash").describe(
                "hash (default) produces a PHC hash string; verify checks the password against the `hash` PHC string.",
            ),
        )
        .param(Param::string("hash").describe("The PHC hash string to verify against (verify mode only)."))
        .param(
            Param::integer("memory_kib")
                .min(8.0)
                .max(1048576.0)
                .describe("Memory cost in KiB (hash mode; default 19456 = 19 MiB, per OWASP)."),
        )
        .param(
            Param::integer("iterations")
                .min(1.0)
                .max(50.0)
                .describe("Iteration (time) cost (hash mode; default 2)."),
        )
        .param(
            Param::integer("parallelism")
                .min(1.0)
                .max(16.0)
                .describe("Parallelism / lanes (hash mode; default 1)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Argon2Hash;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/argon2-hash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hash or verify a password with Argon2id",
    skill(
        description = "Hash a password with Argon2id (the modern, memory-hard password hash) returning a standard PHC string, or verify a password against one. mode=hash (default) uses configurable memory_kib (default 19456 = 19 MiB), iterations (default 2) and parallelism (default 1) and a fresh random salt; mode=verify checks the password against the provided `hash` PHC string. Runs locally — the password never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Argon2Hash {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "argon2-hash", |a: Args| {
            match a.mode.trim().to_ascii_lowercase().as_str() {
                "hash" | "" => {
                    let phc = hash(&a.password, a.memory_kib, a.iterations, a.parallelism)
                        .map_err(SkillError::InvalidArgs)?;
                    Ok(Resp { phc: Some(phc), matches: None })
                }
                "verify" => {
                    if a.hash.trim().is_empty() {
                        return Err(SkillError::InvalidArgs(
                            "verify mode requires the `hash` PHC string".into(),
                        ));
                    }
                    let m = verify(&a.password, &a.hash).map_err(SkillError::InvalidArgs)?;
                    Ok(Resp { phc: None, matches: Some(m) })
                }
                other => Err(SkillError::InvalidArgs(format!(
                    "unknown mode '{other}' (use hash or verify)"
                ))),
            }
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
                    "password": { "type": "string", "description": "The password to hash, or to verify." },
                    "mode": { "type": "string", "enum": ["hash", "verify"], "default": "hash", "description": "hash (default) produces a PHC hash string; verify checks the password against the `hash` PHC string." },
                    "hash": { "type": "string", "description": "The PHC hash string to verify against (verify mode only)." },
                    "memory_kib": { "type": "integer", "minimum": 8, "maximum": 1048576, "description": "Memory cost in KiB (hash mode; default 19456 = 19 MiB, per OWASP)." },
                    "iterations": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Iteration (time) cost (hash mode; default 2)." },
                    "parallelism": { "type": "integer", "minimum": 1, "maximum": 16, "description": "Parallelism / lanes (hash mode; default 1)." }
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
