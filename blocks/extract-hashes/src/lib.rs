//! gizza-ai/extract-hashes — scan text for hexadecimal hash/digest strings and
//! group them by detected algorithm (MD5/SHA-1/SHA-256/SHA-512). Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_hashes_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_true")]
    lowercase: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan for hexadecimal hash strings."),
        )
        .param(
            Param::boolean("lowercase")
                .default(true)
                .describe("When true (default), normalize extracted hashes to lowercase; de-duplication is always case-insensitive."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-hashes",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract hex hash strings from text, grouped by algorithm",
    skill(
        description = "Scan a block of text and pull out every hexadecimal hash/digest string, grouped by the algorithm implied by its length: MD5 (32 hex chars), SHA-1 (40), SHA-256 (64), and SHA-512 (128). Hashes are de-duplicated case-insensitively and kept in first-seen order within each group; set lowercase=false to keep the original casing in the output. Returns the total count and per-algorithm groups (algorithm, length, hashes). Useful for pulling IOCs out of malware reports, mining checksums from logs, or collecting digests from a manifest. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-hashes", |a: Args| {
            Ok::<_, SkillError>(extract(&a.text, a.lowercase))
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
                    "text": { "type": "string", "description": "The text to scan for hexadecimal hash strings." },
                    "lowercase": { "type": "boolean", "default": true, "description": "When true (default), normalize extracted hashes to lowercase; de-duplication is always case-insensitive." }
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
