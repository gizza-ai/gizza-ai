//! gizza-ai/generate-hotp — generate a counter-based one-time code (HOTP,
//! RFC 4226) from a base32 secret. Thin wrapper; chat schema single-sourced
//! from descriptor() (which also drives the CLI); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_generate_hotp_core::{generate, Algo};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    secret: String,
    counter: u64,
    #[serde(default = "default_digits")]
    digits: u32,
    #[serde(default = "default_algo")]
    algorithm: String,
}
fn default_digits() -> u32 {
    6
}
fn default_algo() -> String {
    "sha1".to_string()
}

#[derive(Serialize)]
struct Resp {
    code: String,
    counter: u64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("secret")
                .required()
                .describe("The HOTP shared secret, base32-encoded (spaces and case are ignored)."),
        )
        .param(
            Param::integer("counter")
                .required()
                .min(0.0)
                .describe("The counter value (a non-negative integer); incremented for each new code."),
        )
        .param(
            Param::integer("digits")
                .min(6.0)
                .max(8.0)
                .describe("Number of digits in the code (6, 7, or 8; default 6)."),
        )
        .param(
            Param::enumv("algorithm", ["sha1", "sha256", "sha512"]).default("sha1").describe(
                "HMAC hash algorithm (default sha1, the standard for authenticator apps).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GenerateHotp;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/generate-hotp",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an HOTP (counter-based 2FA) code from a secret",
    skill(
        description = "Generate a counter-based one-time code (HOTP, RFC 4226) from a base32 secret and a counter value. HOTP is event-based: each code is tied to a counter you increment per use (unlike TOTP, which uses the current time). digits (6-8, default 6) and algorithm (sha1 default / sha256 / sha512) are configurable. Returns the code for that counter. Runs locally — the secret never leaves the device.",
        parameters = schema_json()
    ),
)]
impl GenerateHotp {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "generate-hotp", |a: Args| {
            let algo = Algo::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?;
            let r = generate(&a.secret, a.counter, a.digits, algo)
                .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { code: r.code, counter: r.counter })
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
                    "secret": { "type": "string", "description": "The HOTP shared secret, base32-encoded (spaces and case are ignored)." },
                    "counter": { "type": "integer", "minimum": 0, "description": "The counter value (a non-negative integer); incremented for each new code." },
                    "digits": { "type": "integer", "minimum": 6, "maximum": 8, "description": "Number of digits in the code (6, 7, or 8; default 6)." },
                    "algorithm": { "type": "string", "enum": ["sha1", "sha256", "sha512"], "default": "sha1", "description": "HMAC hash algorithm (default sha1, the standard for authenticator apps)." }
                },
                "required": ["secret", "counter"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
