//! gizza-ai/basic-auth-header-generator — build an HTTP Basic Authorization
//! header from a username + password. Thin wrapper around the core; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_basic_auth_header_generator_core::build;
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    full_header: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("username").required().describe("The username (must not contain a colon)."))
        .param(Param::string("password").default("").describe("The password (may be empty)."))
        .param(Param::boolean("full_header").default(false).describe("When true, return the full 'Authorization: Basic …' header line; otherwise just the 'Basic …' value. Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BasicAuthHeaderGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/basic-auth-header-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build an HTTP Basic Authorization header",
    skill(
        description = "Build an HTTP Basic Authorization header value from a username and password: 'Basic ' + base64(username:password) per RFC 7617. Set full_header=true to get the whole 'Authorization: Basic …' line. The username must not contain a colon; the password may be empty.",
        parameters = schema_json()
    )
)]
impl BasicAuthHeaderGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "basic-auth-header-generator", |a: Args| {
            build(&a.username, &a.password, a.full_header).map_err(SkillError::InvalidArgs)
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
                    "username":    { "type": "string", "description": "The username (must not contain a colon)." },
                    "password":    { "type": "string", "default": "", "description": "The password (may be empty)." },
                    "full_header": { "type": "boolean", "default": false, "description": "When true, return the full 'Authorization: Basic …' header line; otherwise just the 'Basic …' value. Default false." }
                },
                "required": ["username"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
