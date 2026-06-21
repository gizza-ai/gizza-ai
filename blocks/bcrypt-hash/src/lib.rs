//! gizza-ai/bcrypt-hash — hash a password with bcrypt (or verify one against an
//! existing bcrypt hash). Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_bcrypt_hash_core::{hash, verify};
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
    #[serde(default = "default_cost")]
    cost: u32,
}
fn default_mode() -> String {
    "hash".to_string()
}
fn default_cost() -> u32 {
    12
}

#[derive(Serialize)]
struct Resp {
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "match")]
    matches: Option<bool>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("password").required().describe("The password to hash, or to verify."))
        .param(
            Param::enumv("mode", ["hash", "verify"]).default("hash").describe(
                "hash (default) produces a bcrypt hash string; verify checks the password against the `hash` string.",
            ),
        )
        .param(Param::string("hash").describe("The bcrypt hash string to verify against (verify mode only)."))
        .param(
            Param::integer("cost")
                .min(4.0)
                .max(31.0)
                .describe("Cost / work factor (hash mode; default 12). Each +1 doubles the work."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BcryptHash;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bcrypt-hash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hash or verify a password with bcrypt",
    skill(
        description = "Hash a password with bcrypt (the long-standing adaptive password hash) returning a standard $2b$ modular-crypt string, or verify a password against an existing bcrypt hash. mode=hash (default) uses a configurable cost / work factor (default 12; each +1 doubles the work) and a fresh random salt; mode=verify checks the password against the provided `hash` string (accepts $2a$/$2b$/$2x$/$2y$ variants). Note: bcrypt only hashes the first 72 bytes of a password. Runs locally — the password never leaves the device.",
        parameters = schema_json()
    ),
)]
impl BcryptHash {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bcrypt-hash", |a: Args| {
            match a.mode.trim().to_ascii_lowercase().as_str() {
                "hash" | "" => {
                    let h = hash(&a.password, a.cost).map_err(SkillError::InvalidArgs)?;
                    Ok(Resp { hash: Some(h), matches: None })
                }
                "verify" => {
                    if a.hash.trim().is_empty() {
                        return Err(SkillError::InvalidArgs(
                            "verify mode requires the `hash` string".into(),
                        ));
                    }
                    let m = verify(&a.password, &a.hash).map_err(SkillError::InvalidArgs)?;
                    Ok(Resp { hash: None, matches: Some(m) })
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
                    "mode": { "type": "string", "enum": ["hash", "verify"], "default": "hash", "description": "hash (default) produces a bcrypt hash string; verify checks the password against the `hash` string." },
                    "hash": { "type": "string", "description": "The bcrypt hash string to verify against (verify mode only)." },
                    "cost": { "type": "integer", "minimum": 4, "maximum": 31, "description": "Cost / work factor (hash mode; default 12). Each +1 doubles the work." }
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
