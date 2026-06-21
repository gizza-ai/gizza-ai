//! gizza-ai/document-splitter — split a long Markdown/HTML document into one
//! section per top-level heading. Chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_document_splitter_core::{split, Format};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    document: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "markdown".to_string()
}

#[derive(Serialize)]
struct Resp {
    count: usize,
    sections: Vec<gizza_ai_document_splitter_core::Section>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("document")
                .required()
                .describe("The Markdown or HTML document to split into per-section files."),
        )
        .param(
            Param::enumv("format", ["markdown", "html"])
                .default("markdown")
                .describe("Document syntax: markdown (split on # headings) or html (split on <hN> tags)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/document-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a Markdown/HTML document into per-section files",
    skill(
        description = "Split a long Markdown or HTML document into separate files, one per top-level section. Markdown splits at the smallest heading level present (e.g. every `#`); HTML splits at the smallest <hN> tag present. Content before the first heading becomes an 'intro' section. Returns each section's suggested filename, title, and content. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "document-splitter", |a: Args| {
            let fmt = Format::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let sections = split(&a.document, fmt).map_err(SkillError::InvalidArgs)?;
            Ok(Resp { count: sections.len(), sections })
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
                    "document": { "type": "string", "description": "The Markdown or HTML document to split into per-section files." },
                    "format": { "type": "string", "enum": ["markdown", "html"], "default": "markdown", "description": "Document syntax: markdown (split on # headings) or html (split on <hN> tags)." }
                },
                "required": ["document"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
