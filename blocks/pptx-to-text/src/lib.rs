//! gizza-ai/pptx-to-text — extract text and a per-slide outline from a modern
//! ZIP-based PowerPoint `.pptx` document. Chat + CLI only (no standalone page):
//! file input in, structured JSON text out.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pptx_to_text_core::{NotesMode, Options, WhitespaceMode, MAX_INPUT_BYTES};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_notes")]
    notes: String,
    #[serde(default = "default_whitespace")]
    whitespace: String,
    #[serde(default = "default_include_hidden")]
    include_hidden: bool,
}

fn default_notes() -> String {
    "include".into()
}
fn default_whitespace() -> String {
    "clean".into()
}
fn default_include_hidden() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("notes", ["include", "exclude", "only"])
                .default("include")
                .describe("Speaker notes handling: include (default) appends notes to each slide's text and includes them in slides[].notes; exclude ignores notes; only extracts speaker notes without slide body text."),
        )
        .param(
            Param::enumv("whitespace", ["clean", "raw"])
                .default("clean")
                .describe("Whitespace handling: clean (default) trims and collapses runs of whitespace inside paragraphs; raw preserves line-ish text from PowerPoint runs more literally."),
        )
        .param(
            Param::boolean("include_hidden")
                .default(true)
                .describe("Whether to include hidden slides marked show=0 in the presentation XML. Default true; set false to omit hidden slides from the outline and combined text."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PptxToText;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pptx-to-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract text and slide outline from a PowerPoint .pptx file.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract text and a per-slide outline from a modern PowerPoint .pptx file (Office Open XML PresentationML). Reads slides in presentation order, captures title/body text, optionally includes speaker notes, can omit hidden slides, and returns structured JSON with combined text, slide_count, word/paragraph counts, and slides[] entries (number, title, text, notes, hidden). This does not fetch live data and does not parse legacy binary .ppt files. Provide the .pptx as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl PptxToText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pptx-to-text")?;
    let notes = NotesMode::parse(&args.notes).map_err(SkillError::InvalidArgs)?;
    let whitespace = WhitespaceMode::parse(&args.whitespace).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _filename) = resolve_source(
        args.source.into_inner(),
        AssetKind::Document,
        MAX_INPUT_BYTES,
    )?;
    let out = gizza_ai_pptx_to_text_core::extract(
        &bytes,
        Options {
            notes,
            whitespace,
            include_hidden: args.include_hidden,
        },
    )
    .map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&out)
        .map_err(|e| SkillError::Serialize(format!("serialize pptx-to-text response: {e}")))
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
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "notes": {
                        "type": "string",
                        "enum": ["include", "exclude", "only"],
                        "default": "include",
                        "description": "Speaker notes handling: include (default) appends notes to each slide's text and includes them in slides[].notes; exclude ignores notes; only extracts speaker notes without slide body text."
                    },
                    "whitespace": {
                        "type": "string",
                        "enum": ["clean", "raw"],
                        "default": "clean",
                        "description": "Whitespace handling: clean (default) trims and collapses runs of whitespace inside paragraphs; raw preserves line-ish text from PowerPoint runs more literally."
                    },
                    "include_hidden": {
                        "type": "boolean",
                        "default": true,
                        "description": "Whether to include hidden slides marked show=0 in the presentation XML. Default true; set false to omit hidden slides from the outline and combined text."
                    }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
