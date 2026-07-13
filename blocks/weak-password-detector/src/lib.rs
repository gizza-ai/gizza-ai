//! gizza-ai/weak-password-detector — check a password against a bundled, ranked
//! list of the most common / breached passwords, entirely offline. Chat schema
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill and returns the structured Detection as JSON. Pure →
//! runs on all backends. This is a bundled blocklist check, NOT a live breach
//! lookup — a "not found" is not proof of strength.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_weak_password_detector_core::detect;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_case_sensitive")]
    case_sensitive: bool,
    #[serde(default = "default_normalize_leet")]
    normalize_leet: bool,
}

fn default_case_sensitive() -> bool {
    false
}
fn default_normalize_leet() -> bool {
    true
}

/// Single source for the chat schema (and CLI). The check is deterministic and
/// offline: it compares the input against a fixed bundled list of the most
/// common / breached passwords, optionally case-insensitively and with common
/// leetspeak substitutions collapsed. Nothing is fetched or invented.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The password to check against the bundled common/breached-password list. Checked locally; never sent anywhere."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("When false (default), 'PASSWORD' matches the listed 'password' — attackers ignore case, so case-only variations are treated as the same weak password. Set true to require an exact-case match."),
        )
        .param(
            Param::boolean("normalize_leet")
                .default(true)
                .describe("When true (default), common leetspeak substitutions are collapsed so 'P@ssw0rd' matches 'password' (0->o, @->a, 1->i, 3->e, 4->a, 5->s, 7->t, etc.). Set false to skip leetspeak normalization."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/weak-password-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check a password against common/breached lists",
    skill(
        description = "Check whether a password is one of the most common or previously-breached passwords, using a bundled ranked blocklist — entirely offline, nothing is sent anywhere. Detects exact matches, case-only variations (case_sensitive=false, the default), and leetspeak variants like P@ssw0rd (normalize_leet=true, the default). Returns found, the 1-based rank (1 = most common), the matched list entry, match_kind (exact/case-insensitive/leetspeak/none), a severity band (critical/high/common/safe), the list size, and a plain-language message. This is a bundled-list dictionary check, NOT a live breach-database (HIBP) lookup: 'not found' rules out well-known weak passwords but is not proof of strength.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "weak-password-detector", |a: Args| {
            detect(&a.input, a.case_sensitive, a.normalize_leet).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The password to check against the bundled common/breached-password list. Checked locally; never sent anywhere." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "When false (default), 'PASSWORD' matches the listed 'password' — attackers ignore case, so case-only variations are treated as the same weak password. Set true to require an exact-case match." },
                    "normalize_leet": { "type": "boolean", "default": true, "description": "When true (default), common leetspeak substitutions are collapsed so 'P@ssw0rd' matches 'password' (0->o, @->a, 1->i, 3->e, 4->a, 5->s, 7->t, etc.). Set false to skip leetspeak normalization." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
