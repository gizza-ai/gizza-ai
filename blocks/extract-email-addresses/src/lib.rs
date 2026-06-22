//! gizza-ai/extract-email-addresses — pull email addresses out of text, dedupe,
//! optionally group by domain. Chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_email_addresses_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    group_by_domain: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan for email addresses."),
        )
        .param(
            Param::boolean("group_by_domain")
                .default(false)
                .describe("When true, also group the addresses by their domain."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractEmailAddresses;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-email-addresses",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pull email addresses out of text",
    skill(
        description = "Scan a block of text and extract all email addresses, deduplicated (case-insensitively) and in first-seen order. Set group_by_domain=true to also get the addresses grouped by their domain. Returns the count, the unique address list, and (optionally) the per-domain grouping. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractEmailAddresses {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-email-addresses", |a: Args| {
            Ok::<_, SkillError>(extract(&a.text, a.group_by_domain))
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
                    "text": { "type": "string", "description": "The text to scan for email addresses." },
                    "group_by_domain": { "type": "boolean", "default": false, "description": "When true, also group the addresses by their domain." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
