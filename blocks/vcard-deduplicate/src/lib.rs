//! gizza-ai/vcard-deduplicate — find and merge duplicate contacts in a vCard
//! (.vcf) file. Thin wrapper; chat schema single-sourced from descriptor() (which
//! also drives the CLI); handle() delegates to block_utils::run_skill. Pure → all
//! backends (chat, CLI, web page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_vcard_deduplicate_core::{run, MatchBy};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_any")]
    match_by: String,
    #[serde(default = "default_true")]
    merge: bool,
}
fn default_any() -> String {
    "any".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The vCard text — one or more .vcf contact cards (each a BEGIN:VCARD … END:VCARD block). vCard 2.1, 3.0, and 4.0 are accepted, including folded lines and multiple cards in one file."),
        )
        .param(
            Param::enumv("match_by", ["any", "name", "email", "phone"])
                .default("any")
                .describe("Which signals mark two cards as the same contact: 'any' groups cards that share a name OR an email OR a phone; 'name' matches only on the full name (FN, else N); 'email' only on a shared email (case-insensitive); 'phone' only on a shared phone (compared by digits). Default 'any'."),
        )
        .param(
            Param::boolean("merge")
                .default(true)
                .describe("With true, each duplicate group is merged into one card — repeatable properties (emails, phones, URLs, addresses) are unioned and empty singular fields are filled from later copies. With false, only the first card of each group is kept and the extra copies are dropped without merging their data. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VcardDeduplicate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vcard-deduplicate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and merge duplicate contacts in a vCard file",
    skill(
        description = "Find and merge duplicate contacts in a vCard (.vcf) file. Groups cards that refer to the same person by matching on names, emails, and/or phone numbers, then either merges each group into one card (unioning phones/emails/URLs/addresses and filling empty fields) or removes the extra copies. Handles vCard 2.1, 3.0, and 4.0, folded lines, and many cards in one file. `match_by` is 'any' (default), 'name', 'email', or 'phone'; `merge` (default true) merges vs. removes. Returns the cleaned vCard text. Runs locally.",
        parameters = schema_json()
    ),
)]
impl VcardDeduplicate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "vcard-deduplicate", |a: Args| {
            let match_by = MatchBy::parse(&a.match_by).map_err(SkillError::InvalidArgs)?;
            run(&a.data, match_by, a.merge).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The vCard text — one or more .vcf contact cards (each a BEGIN:VCARD … END:VCARD block). vCard 2.1, 3.0, and 4.0 are accepted, including folded lines and multiple cards in one file." },
                    "match_by": { "type": "string", "enum": ["any", "name", "email", "phone"], "default": "any", "description": "Which signals mark two cards as the same contact: 'any' groups cards that share a name OR an email OR a phone; 'name' matches only on the full name (FN, else N); 'email' only on a shared email (case-insensitive); 'phone' only on a shared phone (compared by digits). Default 'any'." },
                    "merge": { "type": "boolean", "default": true, "description": "With true, each duplicate group is merged into one card — repeatable properties (emails, phones, URLs, addresses) are unioned and empty singular fields are filled from later copies. With false, only the first card of each group is kept and the extra copies are dropped without merging their data. Default true." }
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
