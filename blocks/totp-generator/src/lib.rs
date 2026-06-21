//! gizza-ai/totp-generator — generate time-based one-time codes (2FA) from a TOTP
//! secret. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_totp_generator_core::{generate, Algo};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    secret: String,
    #[serde(default = "default_digits")]
    digits: u32,
    #[serde(default = "default_period")]
    period: u64,
    #[serde(default = "default_algo")]
    algorithm: String,
    /// Unix time (seconds) to compute the code at; 0 = now.
    #[serde(default)]
    timestamp: u64,
}
fn default_digits() -> u32 {
    6
}
fn default_period() -> u64 {
    30
}
fn default_algo() -> String {
    "sha1".to_string()
}

#[derive(Serialize)]
struct Resp {
    code: String,
    seconds_remaining: u64,
    period: u64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("secret")
                .required()
                .describe("The TOTP shared secret, base32-encoded (spaces and case are ignored)."),
        )
        .param(
            Param::integer("digits")
                .min(6.0)
                .max(8.0)
                .describe("Number of digits in the code (6, 7, or 8; default 6)."),
        )
        .param(
            Param::integer("period")
                .min(1.0)
                .max(600.0)
                .describe("Time step in seconds (default 30)."),
        )
        .param(
            Param::enumv("algorithm", ["sha1", "sha256", "sha512"]).default("sha1").describe(
                "HMAC hash algorithm (default sha1, the standard for authenticator apps).",
            ),
        )
        .param(
            Param::integer("timestamp")
                .min(0.0)
                .describe("Unix time in seconds to compute the code at; omit or 0 for the current time."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
struct TotpGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/totp-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a TOTP (2FA) code from a secret",
    skill(
        description = "Generate a time-based one-time code (TOTP, RFC 6238 — the 2FA codes authenticator apps show) from a base32 secret. Uses the current time by default (pass timestamp to compute a code for a specific Unix time). digits (6-8, default 6), period (seconds, default 30), and algorithm (sha1 default / sha256 / sha512) are configurable. Returns the code and how many seconds it stays valid. Runs locally — the secret never leaves the device.",
        parameters = schema_json()
    ),
)]
impl TotpGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "totp-generator", |a: Args| {
            let algo = Algo::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?;
            let t = if a.timestamp > 0 { a.timestamp } else { now_secs() };
            let r = generate(&a.secret, t, a.digits, a.period, algo)
                .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { code: r.code, seconds_remaining: r.seconds_remaining, period: r.period })
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
                    "secret": { "type": "string", "description": "The TOTP shared secret, base32-encoded (spaces and case are ignored)." },
                    "digits": { "type": "integer", "minimum": 6, "maximum": 8, "description": "Number of digits in the code (6, 7, or 8; default 6)." },
                    "period": { "type": "integer", "minimum": 1, "maximum": 600, "description": "Time step in seconds (default 30)." },
                    "algorithm": { "type": "string", "enum": ["sha1", "sha256", "sha512"], "default": "sha1", "description": "HMAC hash algorithm (default sha1, the standard for authenticator apps)." },
                    "timestamp": { "type": "integer", "minimum": 0, "description": "Unix time in seconds to compute the code at; omit or 0 for the current time." }
                },
                "required": ["secret"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
