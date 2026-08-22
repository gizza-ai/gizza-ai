//! gizza-ai/jwt-weakness-checker — chat skill block on the shared tool abstraction.
//! Offline JWT security audit: flags alg:none, weak/guessable HMAC secrets,
//! missing/expired/over-long expiry, missing best-practice claims, and more,
//! then rolls the findings into a risk score. The chat schema is single-sourced
//! from descriptor() (which also drives the CLI).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    token: String,
    #[serde(default)]
    leeway: i64,
    #[serde(default)]
    now: i64,
    #[serde(default = "default_max_exp_days")]
    max_exp_days: f64,
    #[serde(default)]
    wordlist: String,
}
fn default_max_exp_days() -> f64 {
    30.0
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("token")
                .required()
                .describe("The compact JWT to audit (header.payload.signature)."),
        )
        .param(
            Param::string("wordlist")
                .describe("Extra candidate HMAC secrets to test, in addition to the built-in common-secret list. Separate by newlines or commas. Example: 'secret,changeme,company-name-2024'."),
        )
        .param(
            Param::number("max_exp_days")
                .min(0.0)
                .max(365.0)
                .default(30.0)
                .describe("Lifetime (in days) above which the token's expiry is flagged as excessively long. Default 30; set 0 to disable this check."),
        )
        .param(
            Param::integer("leeway")
                .min(0.0)
                .describe("Clock-skew tolerance in seconds applied to exp/nbf checks (default 0)."),
        )
        .param(
            Param::integer("now")
                .min(0.0)
                .describe("Reference time as seconds since the Unix epoch. When omitted (0), the current clock time is used for expiry checks."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jwt-weakness-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Audit a JWT for security weaknesses offline",
    skill(
        description = "Audit a JSON Web Token (JWT) offline for security weaknesses: the alg:none / unsecured-token trap, weak or guessable HMAC secrets (dictionary attack over a built-in common-secret list plus any you supply), missing/expired/over-long expiry, missing iss/aud/iat/typ claims, kid-injection surface, algorithm-confusion risk, sensitive data in the payload, and oversized tokens. Returns a prioritized findings list with a 0-100 risk score. No secret or key is required and nothing leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jwt-weakness-checker", |a: Args| {
            let now = if a.now == 0 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            } else {
                a.now
            };
            let res = gizza_ai_jwt_weakness_checker_core::audit(
                &a.token,
                now,
                a.leeway,
                a.max_exp_days,
                &a.wordlist,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok::<serde_json::Value, SkillError>(res.to_json())
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
    fn schema_json_matches_expected() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived["type"], "object");
        for p in ["token", "wordlist", "max_exp_days", "leeway", "now"] {
            assert!(
                derived["properties"].get(p).is_some(),
                "missing param {p}"
            );
        }
        // Drift guard: the full schema must match exactly.
        let expected = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "token": {
                    "type": "string",
                    "description": "The compact JWT to audit (header.payload.signature)."
                },
                "wordlist": {
                    "type": "string",
                    "description": "Extra candidate HMAC secrets to test, in addition to the built-in common-secret list. Separate by newlines or commas. Example: 'secret,changeme,company-name-2024'."
                },
                "max_exp_days": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 365,
                    "default": 30.0,
                    "description": "Lifetime (in days) above which the token's expiry is flagged as excessively long. Default 30; set 0 to disable this check."
                },
                "leeway": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Clock-skew tolerance in seconds applied to exp/nbf checks (default 0)."
                },
                "now": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Reference time as seconds since the Unix epoch. When omitted (0), the current clock time is used for expiry checks."
                }
            },
            "required": ["token"]
        });
        assert_eq!(derived, expected, "descriptor schema drifted");
    }
}
