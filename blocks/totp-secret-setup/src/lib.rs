//! gizza-ai/totp-secret-setup — generate a fresh random TOTP secret with its
//! `otpauth://` enrollment URI for authenticator apps. Thin wrapper; the chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure (getrandom CSPRNG + base32) → runs on
//! all backends incl. the chat Service Worker. The secret is generated locally.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_totp_secret_setup_core::generate;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    issuer: String,
    account: String,
    #[serde(default = "default_secret_bytes")]
    secret_bytes: u32,
    #[serde(default = "default_digits")]
    digits: u32,
    #[serde(default = "default_period")]
    period: u64,
    #[serde(default = "default_algorithm")]
    algorithm: String,
}
fn default_secret_bytes() -> u32 { 20 }
fn default_digits() -> u32 { 6 }
fn default_period() -> u64 { 30 }
fn default_algorithm() -> String { "sha1".to_string() }

#[derive(Serialize)]
struct Resp {
    secret: String,
    uri: String,
    bits: usize,
    algorithm: String,
    digits: u32,
    period: u64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("issuer")
                .describe("The provider/organization name shown in the authenticator app (e.g. 'GitHub'). Optional but recommended so the entry is easy to identify."),
        )
        .param(
            Param::string("account")
                .required()
                .describe("The account name / username the codes are for (e.g. an email address). Required; must not contain a colon."),
        )
        .param(
            Param::integer("secret_bytes")
                .default(20)
                .min(10.0)
                .max(64.0)
                .describe("Length of the random secret in bytes (10–64; default 20 = 160 bits, the RFC 4226 recommendation for SHA1)."),
        )
        .param(
            Param::integer("digits")
                .default(6)
                .min(6.0)
                .max(8.0)
                .describe("Number of digits in the generated codes (6, 7, or 8; default 6)."),
        )
        .param(
            Param::integer("period")
                .default(30)
                .min(1.0)
                .describe("TOTP time step in seconds (default 30)."),
        )
        .param(
            Param::enumv("algorithm", ["sha1", "sha256", "sha512"])
                .default("sha1")
                .describe("HMAC hash algorithm (default sha1, the authenticator-app standard). Only change it if the service explicitly requires sha256 or sha512."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/totp-secret-setup",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a random TOTP secret + otpauth:// enrollment URI",
    skill(
        description = "Generate a fresh, cryptographically random TOTP (2FA) secret locally and assemble its standard `otpauth://` enrollment URI for authenticator apps (Google Authenticator, Authy, 1Password, etc.). Set `issuer` (recommended) and `account` (required) to label the entry; `secret_bytes` (10–64, default 20 = 160 bits) sets the secret strength; `digits` (6–8, default 6), `period` (default 30 s), and `algorithm` (sha1/sha256/sha512, default sha1) match what the service expects. Returns the base32 secret, the otpauth:// URI, and the secret strength in bits. The secret is generated on-device and never sent to a server.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "totp-secret-setup", |a: Args| {
            let s = generate(
                &a.issuer,
                &a.account,
                a.secret_bytes as usize,
                a.digits,
                a.period,
                &a.algorithm,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                secret: s.secret,
                uri: s.uri,
                bits: s.bits,
                algorithm: s.algorithm,
                digits: s.digits,
                period: s.period,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "issuer": { "type": "string", "description": "The provider/organization name shown in the authenticator app (e.g. 'GitHub'). Optional but recommended so the entry is easy to identify." },
                    "account": { "type": "string", "description": "The account name / username the codes are for (e.g. an email address). Required; must not contain a colon." },
                    "secret_bytes": { "type": "integer", "minimum": 10, "maximum": 64, "default": 20, "description": "Length of the random secret in bytes (10–64; default 20 = 160 bits, the RFC 4226 recommendation for SHA1)." },
                    "digits": { "type": "integer", "minimum": 6, "maximum": 8, "default": 6, "description": "Number of digits in the generated codes (6, 7, or 8; default 6)." },
                    "period": { "type": "integer", "minimum": 1, "default": 30, "description": "TOTP time step in seconds (default 30)." },
                    "algorithm": { "type": "string", "enum": ["sha1", "sha256", "sha512"], "default": "sha1", "description": "HMAC hash algorithm (default sha1, the authenticator-app standard). Only change it if the service explicitly requires sha256 or sha512." }
                },
                "required": ["account"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
