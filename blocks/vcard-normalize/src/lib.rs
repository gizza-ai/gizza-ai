//! gizza-ai/vcard-normalize — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute, no host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_vcard_normalize_core::NameCase;
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    default_country: String,
    #[serde(default)]
    name_case: String,
    #[serde(default = "default_true")]
    lowercase_email: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("Raw vCard / .vcf text containing one or more BEGIN:VCARD ... END:VCARD blocks. Folded lines are unfolded before normalization."),
        )
        .param(
            Param::string("default_country")
                .default("")
                .describe("Optional ISO-3166 alpha-2 country/region hint (for example US, GB, DE) used to parse phone numbers that do not start with '+'. Leave blank to only normalize already-international numbers."),
        )
        .param(
            Param::enumv("name_case", ["keep", "title", "upper", "lower"])
                .default("keep")
                .describe("How to recase FN, N, and NICKNAME values after whitespace cleanup. 'keep' (default) preserves case; 'title' title-cases each word; 'upper' uppercases; 'lower' lowercases."),
        )
        .param(
            Param::boolean("lowercase_email")
                .default(true)
                .describe("Trim and lowercase EMAIL values. Default true; set false to preserve mailbox case while still trimming surrounding spaces."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vcard-normalize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize emails, phone numbers, and names in vCard address books.",
    skill(
        description = "Normalize a vCard/.vcf address book while preserving unknown properties and card structure. EMAIL values are trimmed and lowercased by default. TEL values are conservatively reformatted toward E.164 when they parse and validate, using optional default_country for national numbers. FN, N, and NICKNAME whitespace is tidied and can be recased with name_case=keep/title/upper/lower. Folded lines are unfolded, CRLF/LF style is preserved, invalid phone numbers are left untouched, and malformed input without a vCard returns an error.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "vcard-normalize", |a: Args| {
            let case = NameCase::parse(&a.name_case).map_err(SkillError::InvalidArgs)?;
            gizza_ai_vcard_normalize_core::run(
                &a.data,
                &a.default_country,
                case,
                a.lowercase_email,
            )
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "Raw vCard / .vcf text containing one or more BEGIN:VCARD ... END:VCARD blocks. Folded lines are unfolded before normalization." },
                    "default_country": { "type": "string", "default": "", "description": "Optional ISO-3166 alpha-2 country/region hint (for example US, GB, DE) used to parse phone numbers that do not start with '+'. Leave blank to only normalize already-international numbers." },
                    "name_case": { "type": "string", "enum": ["keep", "title", "upper", "lower"], "default": "keep", "description": "How to recase FN, N, and NICKNAME values after whitespace cleanup. 'keep' (default) preserves case; 'title' title-cases each word; 'upper' uppercases; 'lower' lowercases." },
                    "lowercase_email": { "type": "boolean", "default": true, "description": "Trim and lowercase EMAIL values. Default true; set false to preserve mailbox case while still trimming surrounding spaces." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}