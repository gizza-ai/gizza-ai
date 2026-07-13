//! gizza-ai/contact-info-extractor — pull email addresses and phone numbers out
//! of unstructured text, deduplicate, and count. Chat schema single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_contact_info_extractor_core::extract_str;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_types")]
    types: String,
    #[serde(default = "default_dedupe")]
    dedupe: bool,
    #[serde(default = "default_sort")]
    sort: String,
}

fn default_types() -> String {
    "both".into()
}
fn default_dedupe() -> bool {
    true
}
fn default_sort() -> String {
    "first-seen".into()
}

/// Single source for the chat schema (and CLI). Extraction is deterministic: a
/// pragmatic email regex plus a multi-format phone matcher (international `+CC`,
/// `(area)`, dashed/dotted/spaced/continuous). Nothing is invented.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The unstructured text to scan for email addresses and phone numbers (email threads, signatures, contact lists, exported notes)."),
        )
        .param(
            Param::enumv("types", ["both", "emails", "phones"])
                .default("both")
                .describe("Which contact types to extract: 'both' (default), 'emails' only, or 'phones' only."),
        )
        .param(
            Param::boolean("dedupe")
                .default(true)
                .describe("Remove duplicate values. Emails are deduplicated case-insensitively; phone numbers by their normalized digits. Turn off to keep every occurrence."),
        )
        .param(
            Param::enumv("sort", ["first-seen", "alphabetical"])
                .default("first-seen")
                .describe("Ordering of each list: 'first-seen' keeps the order values appear in the text; 'alphabetical' sorts ascending."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/contact-info-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract emails and phone numbers from text",
    skill(
        description = "Scan a block of unstructured text and pull out every email address and phone number, deduplicated and counted. Emails dedupe case-insensitively; phone numbers dedupe by normalized digits and are detected across international (+CC), (area), dashed, dotted, spaced, and continuous formats. Set types to 'emails' or 'phones' to narrow the scan, dedupe=false to keep duplicates, and sort to 'alphabetical'. Returns email_count, phone_count, total, and the email/phone lists. Deterministic regex, no LLM, runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "contact-info-extractor", |a: Args| {
            extract_str(&a.input, &a.types, a.dedupe, &a.sort).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The unstructured text to scan for email addresses and phone numbers (email threads, signatures, contact lists, exported notes)." },
                    "types": { "type": "string", "enum": ["both", "emails", "phones"], "default": "both", "description": "Which contact types to extract: 'both' (default), 'emails' only, or 'phones' only." },
                    "dedupe": { "type": "boolean", "default": true, "description": "Remove duplicate values. Emails are deduplicated case-insensitively; phone numbers by their normalized digits. Turn off to keep every occurrence." },
                    "sort": { "type": "string", "enum": ["first-seen", "alphabetical"], "default": "first-seen", "description": "Ordering of each list: 'first-seen' keeps the order values appear in the text; 'alphabetical' sorts ascending." }
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
