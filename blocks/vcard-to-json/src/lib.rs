//! gizza-ai/vcard-to-json — parse vCard (.vcf) contact cards into a JSON array of
//! contacts. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends (chat, CLI, web page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_vcard_to_json_core::{run, Wrap};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    structured: bool,
    #[serde(default = "default_array")]
    wrap: String,
    #[serde(default)]
    pretty: bool,
}
fn default_true() -> bool {
    true
}
fn default_array() -> String {
    "array".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The vCard text — one or more .vcf contact cards (each a BEGIN:VCARD … END:VCARD block). vCard 2.1, 3.0, and 4.0 are accepted."),
        )
        .param(
            Param::boolean("structured")
                .default(true)
                .describe("Split the Name (N) into family/given/middle/prefix/suffix and each Address (ADR) into poBox/extended/street/locality/region/postalCode/country. With false, N and ADR stay as their raw semicolon-joined strings. Default true."),
        )
        .param(
            Param::enumv("wrap", ["array", "object"])
                .default("array")
                .describe("Top-level shape: 'array' returns a bare JSON array of contact objects; 'object' returns { \"contacts\": [...], \"count\": N }. Default 'array'."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Pretty-print (indent) the JSON output instead of emitting it on one line. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VcardToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vcard-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert vCard (.vcf) contacts to a JSON array",
    skill(
        description = "Parse vCard (.vcf) contact cards into a JSON array of contacts. Accepts vCard 2.1, 3.0, and 4.0, handling folded continuation lines, multiple cards in one file, repeatable properties (emails/phones/urls/addresses become arrays with their TYPE tags), group prefixes, and value unescaping. `structured` (default true) splits Name and Address into component fields; `wrap` is 'array' (bare array) or 'object' ({contacts,count}); `pretty` indents the JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl VcardToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "vcard-to-json", |a: Args| {
            let wrap = Wrap::parse(&a.wrap).map_err(SkillError::InvalidArgs)?;
            run(&a.data, a.structured, wrap, a.pretty).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The vCard text — one or more .vcf contact cards (each a BEGIN:VCARD … END:VCARD block). vCard 2.1, 3.0, and 4.0 are accepted." },
                    "structured": { "type": "boolean", "default": true, "description": "Split the Name (N) into family/given/middle/prefix/suffix and each Address (ADR) into poBox/extended/street/locality/region/postalCode/country. With false, N and ADR stay as their raw semicolon-joined strings. Default true." },
                    "wrap": { "type": "string", "enum": ["array", "object"], "default": "array", "description": "Top-level shape: 'array' returns a bare JSON array of contact objects; 'object' returns { \"contacts\": [...], \"count\": N }. Default 'array'." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print (indent) the JSON output instead of emitting it on one line. Default false." }
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
