//! gizza-ai/otpauth-uri — build a standard `otpauth://` provisioning URI (Key Uri
//! Format) from an issuer, account, and base32 secret, for setting up an authenticator
//! app. Pure → all backends. Chat schema single-sourced from descriptor(); handle()
//! delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_otpauth_uri_core::{build, Algo, OtpAuth, OtpType};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_type")]
    r#type: String,
    #[serde(default)]
    issuer: String,
    account: String,
    secret: String,
    #[serde(default = "default_digits")]
    digits: u32,
    #[serde(default = "default_period")]
    period: u64,
    #[serde(default = "default_algo")]
    algorithm: String,
    #[serde(default)]
    counter: u64,
}
fn default_type() -> String {
    "totp".to_string()
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
    uri: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("type", ["totp", "hotp"]).default("totp").describe(
                "OTP type: totp (time-based, default — the usual 2FA codes) or hotp (counter-based).",
            ),
        )
        .param(
            Param::string("issuer")
                .describe("The provider/organization name shown in the authenticator app (e.g. 'GitHub'). Recommended."),
        )
        .param(
            Param::string("account")
                .required()
                .describe("The account name / username the code is for (e.g. an email address). Must not contain a colon."),
        )
        .param(
            Param::string("secret")
                .required()
                .describe("The shared secret, base32-encoded (RFC 4648; spaces and case are ignored)."),
        )
        .param(
            Param::integer("digits")
                .min(6.0)
                .max(8.0)
                .describe("Number of digits in the generated codes (6, 7, or 8; default 6)."),
        )
        .param(
            Param::integer("period")
                .min(1.0)
                .max(600.0)
                .describe("TOTP time step in seconds (default 30; ignored for hotp)."),
        )
        .param(
            Param::enumv("algorithm", ["sha1", "sha256", "sha512"]).default("sha1").describe(
                "HMAC hash algorithm (default sha1, the standard for authenticator apps).",
            ),
        )
        .param(
            Param::integer("counter")
                .min(0.0)
                .describe("HOTP initial counter value (default 0; ignored for totp)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_uri(a: Args) -> Result<Resp, String> {
    let otp_type = OtpType::parse(&a.r#type)?;
    let algorithm = Algo::parse(&a.algorithm)?;
    let uri = build(&OtpAuth {
        otp_type,
        issuer: a.issuer,
        account: a.account,
        secret: a.secret,
        algorithm,
        digits: a.digits,
        period: a.period,
        counter: a.counter,
    })?;
    Ok(Resp { uri })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/otpauth-uri",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build an otpauth:// provisioning URI for an authenticator app",
    skill(
        description = "Build a standard otpauth:// provisioning URI (the Key Uri Format used by Google Authenticator, Authy, 1Password, and other authenticator apps) from an issuer, account name, and base32 secret. Encode this URI as a QR code and scan it to set up 2FA. Supports totp (time-based, default) and hotp (counter-based), with configurable digits (6-8), period (seconds), algorithm (sha1 default / sha256 / sha512), and HOTP counter. Returns the otpauth:// URI string. Runs locally — the secret never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "otpauth-uri", |a: Args| {
            build_uri(a).map_err(SkillError::InvalidArgs)
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
                    "type": { "type": "string", "enum": ["totp", "hotp"], "default": "totp", "description": "OTP type: totp (time-based, default — the usual 2FA codes) or hotp (counter-based)." },
                    "issuer": { "type": "string", "description": "The provider/organization name shown in the authenticator app (e.g. 'GitHub'). Recommended." },
                    "account": { "type": "string", "description": "The account name / username the code is for (e.g. an email address). Must not contain a colon." },
                    "secret": { "type": "string", "description": "The shared secret, base32-encoded (RFC 4648; spaces and case are ignored)." },
                    "digits": { "type": "integer", "minimum": 6, "maximum": 8, "description": "Number of digits in the generated codes (6, 7, or 8; default 6)." },
                    "period": { "type": "integer", "minimum": 1, "maximum": 600, "description": "TOTP time step in seconds (default 30; ignored for hotp)." },
                    "algorithm": { "type": "string", "enum": ["sha1", "sha256", "sha512"], "default": "sha1", "description": "HMAC hash algorithm (default sha1, the standard for authenticator apps)." },
                    "counter": { "type": "integer", "minimum": 0, "description": "HOTP initial counter value (default 0; ignored for totp)." }
                },
                "required": ["account", "secret"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
