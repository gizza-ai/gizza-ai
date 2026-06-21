//! gizza-ai/jwt-verify — chat skill block on the shared tool abstraction.
//! Verifies a JWT's signature (HS*/RS*/ES*) and validates exp/nbf/iss/aud.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jwt_verify_core::{verify, Alg, Options};
use serde::Deserialize;
use serde_json::Value;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    token: String,
    key: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    issuer: String,
    #[serde(default)]
    audience: String,
    #[serde(default)]
    leeway: i64,
    /// Current time in seconds since the Unix epoch. The chat backend supplies
    /// a clock; when omitted (0) exp/nbf are still reported but evaluated at
    /// epoch 0 — callers should pass `now` for time-claim validation.
    #[serde(default)]
    now: i64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("token")
                .required()
                .describe("The compact JWT to verify, i.e. the header.payload.signature string."),
        )
        .param(Param::string("key").required().describe(
            "The verification key: for HS* the shared HMAC secret string; for RS*/ES* a PEM-encoded PUBLIC key ('-----BEGIN PUBLIC KEY-----' SPKI, or '-----BEGIN RSA PUBLIC KEY-----' PKCS#1 for RSA).",
        ))
        .param(
            Param::enumv(
                "algorithm",
                ["", "HS256", "HS384", "HS512", "RS256", "RS384", "RS512", "ES256", "ES384"],
            )
            .describe("Optional required algorithm — if set, the token's 'alg' header must equal it (defends against algorithm-confusion attacks). Leave empty to accept the token's own alg (except 'none', which is always rejected)."),
        )
        .param(Param::string("issuer").describe(
            "Optional expected 'iss' claim. If set, verification fails unless the token's issuer matches exactly.",
        ))
        .param(Param::string("audience").describe(
            "Optional expected 'aud' claim. If set, verification fails unless the token's audience (string or array) contains this value.",
        ))
        .param(
            Param::integer("leeway")
                .min(0.0)
                .describe("Clock-skew tolerance in seconds applied to exp/nbf (default 0)."),
        )
        .param(
            Param::integer("now")
                .min(0.0)
                .describe("Current time as seconds since the Unix epoch, used for exp/nbf checks. The chat client supplies this automatically; pass it explicitly when calling directly."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JwtVerify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jwt-verify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Verify a JSON Web Token (JWT) signature and claims",
    skill(
        description = "Verify a JSON Web Token (JWT, JWS compact serialization): check its signature against a key and validate the standard exp/nbf/iss/aud claims. Set algorithm=HS256/384/512 (HMAC with a shared secret), RS256/384/512 (RSASSA-PKCS1-v1_5 with a PEM RSA public key) or ES256/384 (ECDSA with a PEM P-256/P-384 public key) to REQUIRE that algorithm (recommended; defends against alg-confusion) — leave it empty to use the token's own alg ('none' is always rejected). 'token' is the compact JWT; 'key' is the HMAC secret (HS*) or PEM public key (RS*/ES*); optional 'issuer'/'audience' assert the iss/aud claims; 'leeway' tolerates clock skew on exp/nbf; 'now' is the current Unix time for time checks. Returns {valid, algorithm, error, header, payload, checks[]}. Runs locally — the token and key never leave the device.",
        parameters = schema_json()
    ),
)]
impl JwtVerify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jwt-verify", |a: Args| {
            let expected_alg = if a.algorithm.trim().is_empty() {
                None
            } else {
                Some(Alg::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?)
            };
            let opts = Options {
                expected_alg,
                expected_iss: non_empty(a.issuer),
                expected_aud: non_empty(a.audience),
                now: a.now,
                leeway: a.leeway,
            };
            let res = verify(&a.token, &a.key, &opts).map_err(SkillError::InvalidArgs)?;
            Ok::<Value, SkillError>(res.to_json())
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
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
                    "token": { "type": "string", "description": "The compact JWT to verify, i.e. the header.payload.signature string." },
                    "key": { "type": "string", "description": "The verification key: for HS* the shared HMAC secret string; for RS*/ES* a PEM-encoded PUBLIC key ('-----BEGIN PUBLIC KEY-----' SPKI, or '-----BEGIN RSA PUBLIC KEY-----' PKCS#1 for RSA)." },
                    "algorithm": { "type": "string", "enum": ["", "HS256", "HS384", "HS512", "RS256", "RS384", "RS512", "ES256", "ES384"], "description": "Optional required algorithm — if set, the token's 'alg' header must equal it (defends against algorithm-confusion attacks). Leave empty to accept the token's own alg (except 'none', which is always rejected)." },
                    "issuer": { "type": "string", "description": "Optional expected 'iss' claim. If set, verification fails unless the token's issuer matches exactly." },
                    "audience": { "type": "string", "description": "Optional expected 'aud' claim. If set, verification fails unless the token's audience (string or array) contains this value." },
                    "leeway": { "type": "integer", "minimum": 0, "description": "Clock-skew tolerance in seconds applied to exp/nbf (default 0)." },
                    "now": { "type": "integer", "minimum": 0, "description": "Current time as seconds since the Unix epoch, used for exp/nbf checks. The chat client supplies this automatically; pass it explicitly when calling directly." }
                },
                "required": ["token", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
