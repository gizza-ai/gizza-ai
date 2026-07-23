//! gizza-ai/mbox-dedup — remove duplicate messages from an mbox archive by their
//! Message-ID header, keeping the first or last occurrence. Chat schema
//! single-sourced from descriptor(); handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mbox_dedup_core::{dedupe, Keep, NoId, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    mbox: String,
    #[serde(default = "default_keep")]
    keep: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "default_no_message_id")]
    no_message_id: String,
}

fn default_keep() -> String {
    "first".to_string()
}

fn default_no_message_id() -> String {
    "keep".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("mbox")
                .required()
                .describe("The mbox archive text to de-duplicate. Messages are delimited by the classic `From ` postmark lines at the start of a line; a single pasted RFC 5322 message (no postmark) is treated as one message. Surviving messages are returned verbatim."),
        )
        .param(
            Param::enumv("keep", ["first", "last"])
                .default("first")
                .describe("Which copy of each duplicated Message-ID to keep: the first occurrence (default) or the last. The surviving messages stay in their original order either way."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare Message-IDs case-insensitively. RFC 5322 Message-IDs are case-sensitive, so this is off by default; turn it on if an exporter changed the case of otherwise-identical IDs."),
        )
        .param(
            Param::enumv("no_message_id", ["keep", "drop"])
                .default("keep")
                .describe("What to do with a message that has no Message-ID header (e.g. drafts): keep every such message (default; distinct ID-less messages are never merged together) or drop them all."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MboxDedup;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mbox-dedup",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove duplicate mbox messages by Message-ID",
    skill(
        description = "Find and remove duplicate messages in an mbox archive by their Message-ID header, returning the de-duplicated mbox text plus a summary (total/kept/removed message counts, how many had no Message-ID, and how many IDs were duplicated). Messages are split on the classic `From ` postmark lines and each surviving message is preserved verbatim. By default the first occurrence of each Message-ID is kept and original order is preserved; set keep=\"last\" to keep the last. Message-IDs are normalized (surrounding angle brackets and whitespace stripped) and compared case-sensitively unless ignore_case=true. Messages lacking a Message-ID are kept by default (never merged); set no_message_id=\"drop\" to remove them. Runs locally.",
        parameters = schema_json()
    ),
)]
impl MboxDedup {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mbox-dedup", |a: Args| {
            let keep = Keep::parse(&a.keep).map_err(SkillError::InvalidArgs)?;
            let no_id = NoId::parse(&a.no_message_id).map_err(SkillError::InvalidArgs)?;
            let opts = Options { keep, ignore_case: a.ignore_case, no_id };
            Ok(dedupe(&a.mbox, &opts))
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
                    "mbox": { "type": "string", "description": "The mbox archive text to de-duplicate. Messages are delimited by the classic `From ` postmark lines at the start of a line; a single pasted RFC 5322 message (no postmark) is treated as one message. Surviving messages are returned verbatim." },
                    "keep": { "type": "string", "enum": ["first", "last"], "default": "first", "description": "Which copy of each duplicated Message-ID to keep: the first occurrence (default) or the last. The surviving messages stay in their original order either way." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare Message-IDs case-insensitively. RFC 5322 Message-IDs are case-sensitive, so this is off by default; turn it on if an exporter changed the case of otherwise-identical IDs." },
                    "no_message_id": { "type": "string", "enum": ["keep", "drop"], "default": "keep", "description": "What to do with a message that has no Message-ID header (e.g. drafts): keep every such message (default; distinct ID-less messages are never merged together) or drop them all." }
                },
                "required": ["mbox"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
