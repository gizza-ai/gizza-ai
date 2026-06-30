//! gizza-ai/structured-data-validator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_structured_data_validator_core::{parse_format, validate};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML source to scan for structured data (paste the <head> and <body>)."),
        )
        .param(
            Param::enumv("format", ["report", "json"])
                .default("report")
                .describe("Output: 'report' (human-readable, default) or 'json' (machine-readable summary)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/structured-data-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate JSON-LD, microdata and RDFa structured data in HTML",
    skill(
        description = "Extract and validate structured data (JSON-LD, microdata, and RDFa) from pasted HTML. Lists every schema.org entity it finds with its @type and properties, and flags common mistakes: invalid JSON-LD, a missing or non-schema.org @context, objects with no @type, itemprop attributes outside any itemscope, an itemscope with no itemtype, and a typeof with no vocab. Set format='report' (default, human-readable) or 'json' (a machine-readable {counts, items, issues, valid} summary).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "structured-data-validator", |a: Args| {
            let fmt = parse_format(&a.format).map_err(SkillError::InvalidArgs)?;
            validate(&a.html, fmt).map_err(SkillError::InvalidArgs)
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
                    "html":   { "type": "string", "description": "The HTML source to scan for structured data (paste the <head> and <body>)." },
                    "format": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output: 'report' (human-readable, default) or 'json' (machine-readable summary)." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
