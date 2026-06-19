//! gizza-ai/phone-format — chat skill block (thin wrapper around core).
//!
//! Parses, validates, and formats an international phone number. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill` and returns
//! `{ "result": "Valid: yes\nE.164: …\n…" }`. No host calls — runs entirely
//! inside the WASM sandbox (the `phonenumber` crate bundles its own metadata).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    number: String,
    #[serde(default)]
    region: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("number")
                .required()
                .describe("The phone number to analyze. May include a +country prefix, e.g. +1 415 555 2671."),
        )
        .param(
            Param::string("region")
                .describe("Optional ISO-3166 alpha-2 region code (e.g. US, GB, DE) used to interpret a number given without a + prefix."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PhoneFormat;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/phone-format",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Phone number formatter & validator skill",
    skill(
        description = "Parse, validate, and format an international phone number. Reports validity, E.164, national & international formats, the country/region, and the number type when derivable.",
        parameters = schema_json()
    )
)]
impl PhoneFormat {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } — phone-format's
        // existing success shape — and routes errors through GuestResult::error.
        match run_skill(&body, "phone-format", |a: Args| {
            gizza_ai_phone_format_core::format_number(&a.number, &a.region)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. (to_schema_json
    /// now emits `additionalProperties: false` uniformly, which phone-format's
    /// authored schema already had — so this is an exact match.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "number": { "type": "string", "description": "The phone number to analyze. May include a +country prefix, e.g. +1 415 555 2671." },
                    "region": { "type": "string", "description": "Optional ISO-3166 alpha-2 region code (e.g. US, GB, DE) used to interpret a number given without a + prefix." }
                },
                "required": ["number"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
