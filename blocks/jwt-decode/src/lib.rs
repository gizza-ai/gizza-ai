//! gizza-ai/jwt-decode — chat skill block on the shared tool abstraction.
//! Decodes a JSON Web Token offline without a verification key, validating standard claims.
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
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("token")
                .required()
                .describe("The compact JWT to decode (header.payload.signature)."),
        )
        .param(
            Param::integer("leeway")
                .min(0.0)
                .describe("Clock-skew tolerance in seconds applied to exp/nbf/iat checks (default 0)."),
        )
        .param(
            Param::integer("now")
                .min(0.0)
                .describe("Reference time as seconds since the Unix epoch. When omitted (0), the current clock time is used."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jwt-decode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a JSON Web Token offline without verification",
    skill(
        description = "Decode a JSON Web Token (JWT) offline to inspect its header, payload claims, signature presence, and validate exp/nbf/iat time claims. No secret/public key is required.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jwt-decode", |a: Args| {
            let now = if a.now == 0 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            } else {
                a.now
            };
            let res = gizza_ai_jwt_decode_core::decode_jwt(&a.token, now, a.leeway)
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
        assert!(derived["properties"].get("token").is_some());
        assert!(derived["properties"].get("leeway").is_some());
        assert!(derived["properties"].get("now").is_some());
    }
}
